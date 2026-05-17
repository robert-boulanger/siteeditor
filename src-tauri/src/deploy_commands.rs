//! Tauri-Commands für Phase 10 — Deployment.
//!
//! Dünner Glue zwischen Frontend (React-Modals) und der Logik in
//! `projectfs` (Profile-Persistenz), `keystore` (Secret) und dem
//! konkreten Adapter (`deploy-sftp`).
//!
//! Alle Commands arbeiten über die `AppState`-Project-Referenz — es
//! gibt keinen `project_path`-Parameter, der die Konsistenz aushebeln
//! könnte.

use crate::{to_str_err, AppState};
use deploy_contract::{
    AuthMethod, DeployProfile, DiffReport, Manifest, ProgressEvent, ProgressSink, Protocol,
    UploadPlan, Uploader, diff,
};
use deploy_ftp::FtpAdapter;
use deploy_sftp::SftpAdapter;
use projectfs::Project;
use serde::Serialize;
use tauri::{Emitter, State};

/// Event-Name für Progress-Push ans Frontend.
pub const PROGRESS_EVENT: &str = "deploy://progress";

/// Frontend-Payload spiegelt `ProgressEvent` 1:1 (serde-tauglich).
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ProgressPayload {
    Connected,
    DiffResolved { upload_count: usize, upload_bytes: u64 },
    FileStart { rel_path: String, size: u64 },
    FileDone { rel_path: String },
    ManifestWritten,
    Done { uploaded: usize, total_bytes: u64 },
    Error { message: String },
}

impl From<ProgressEvent> for ProgressPayload {
    fn from(ev: ProgressEvent) -> Self {
        match ev {
            ProgressEvent::Connected => Self::Connected,
            ProgressEvent::DiffResolved { upload_count, upload_bytes } => {
                Self::DiffResolved { upload_count, upload_bytes }
            }
            ProgressEvent::FileStart { rel_path, size } => Self::FileStart { rel_path, size },
            ProgressEvent::FileDone { rel_path } => Self::FileDone { rel_path },
            ProgressEvent::ManifestWritten => Self::ManifestWritten,
            ProgressEvent::Done { uploaded, total_bytes } => Self::Done { uploaded, total_bytes },
        }
    }
}

/// Frontend-Payload für `DiffReport`. Wir reichen die Felder direkt
/// durch — Felder sind alle serde-fähig.
#[derive(Serialize)]
pub struct DiffReportDto {
    strategy: String,
    reason: Option<String>,
    upload: Vec<String>,
    orphan_remote: Vec<String>,
    upload_bytes: u64,
}

impl From<DiffReport> for DiffReportDto {
    fn from(r: DiffReport) -> Self {
        use deploy_contract::DiffStrategy;
        let (strategy, reason) = match &r.strategy {
            DiffStrategy::Incremental => ("incremental".to_string(), None),
            DiffStrategy::Full { reason } => ("full".to_string(), Some(reason.clone())),
        };
        Self {
            strategy,
            reason,
            upload: r.upload,
            orphan_remote: r.orphan_remote,
            upload_bytes: r.upload_bytes,
        }
    }
}

/// Hilfs-Wrapper: liefert das Project oder einen UI-Fehler.
fn with_project<R>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&Project) -> Result<R, String>,
) -> Result<R, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    f(project)
}

fn with_project_mut<R>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut Project) -> Result<R, String>,
) -> Result<R, String> {
    let mut guard = state.project.lock().unwrap();
    let project = guard.as_mut().ok_or_else(|| "no project open".to_string())?;
    f(project)
}

#[tauri::command]
pub fn deploy_list_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<DeployProfile>, String> {
    with_project(&state, |p| Ok(p.list_deploy_profiles()))
}

#[tauri::command]
pub fn deploy_save_profile(
    state: State<'_, AppState>,
    profile: DeployProfile,
    secret: Option<String>,
) -> Result<(), String> {
    // 1. Profil schreiben (validiert intern).
    let stored = with_project_mut(&state, |p| {
        p.upsert_deploy_profile(profile.clone()).map_err(to_str_err)?;
        Ok(profile.clone())
    })?;
    // 2. Falls Secret mitgegeben: speichern. Sonst alten Eintrag bewusst
    //    *nicht* anfassen (User kann das Profil editieren ohne Re-Eingabe).
    if let Some(s) = secret {
        with_project(&state, |p| {
            crate::keystore::store_secret(p, &stored, &s).map_err(to_str_err)
        })?;
    }
    Ok(())
}

#[tauri::command]
pub fn deploy_delete_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    // Erst Secret löschen (braucht das Profil noch in der Liste, um den
    // korrekten Keystore-Key zu kennen). Fehler beim Secret-Löschen ist
    // kein Hard-Fail — Profil rauszuräumen ist wichtiger.
    let profile = with_project(&state, |p| Ok(p.get_deploy_profile(&name)))?;
    if let Some(prof) = profile {
        let _ = with_project(&state, |p| {
            crate::keystore::delete_secret(p, &prof).map_err(to_str_err)
        });
    }
    with_project_mut(&state, |p| p.delete_deploy_profile(&name).map_err(to_str_err))
}

#[tauri::command]
pub fn deploy_has_secret(state: State<'_, AppState>, name: String) -> Result<bool, String> {
    with_project(&state, |p| {
        let prof = p
            .get_deploy_profile(&name)
            .ok_or_else(|| format!("Profil `{name}` nicht gefunden"))?;
        let secret = crate::keystore::load_secret(p, &prof).map_err(to_str_err)?;
        Ok(secret.is_some())
    })
}

/// Build + Diff berechnen, **nichts hochladen**. UI ruft das für die
/// Dry-Run-Anzeige.
#[tauri::command]
pub fn deploy_dry_run(
    state: State<'_, AppState>,
    name: String,
) -> Result<DiffReportDto, String> {
    let (profile, build_dir, secret) = ensure_built_and_load(&state, &name)?;
    let local = Manifest::from_directory(&build_dir).map_err(to_str_err)?;

    let remote = match profile.protocol {
        Protocol::Sftp => {
            let mut adapter = SftpAdapter::new(secret);
            adapter.fetch_remote_manifest(&profile).map_err(to_str_err)?
        }
        Protocol::Ftp => {
            let mut adapter = FtpAdapter::new(secret);
            adapter.fetch_remote_manifest(&profile).map_err(to_str_err)?
        }
        // GitHub-Pages-Adapter kommt in Schritt 8. Bis dahin: Full-Upload.
        Protocol::GithubPages => None,
    };
    Ok(diff::compute(&local, remote.as_ref(), profile.prefer_diff).into())
}

#[tauri::command]
pub fn deploy_run(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let (profile, build_dir, secret) = ensure_built_and_load(&state, &name)?;

    if profile.protocol == Protocol::GithubPages {
        return Err("GitHub-Pages-Adapter ist noch nicht implementiert (Schritt 8).".into());
    }

    let local = Manifest::from_directory(&build_dir).map_err(to_str_err)?;
    let mut adapter: Box<dyn Uploader> = match profile.protocol {
        Protocol::Sftp => Box::new(SftpAdapter::new(secret)),
        Protocol::Ftp => Box::new(FtpAdapter::new(secret)),
        Protocol::GithubPages => unreachable!("oben mit Early-Return abgefangen"),
    };

    // Connect + Manifest holen (UI sieht Connected danach).
    let emit_one = |ev: ProgressEvent| {
        let _ = app.emit(PROGRESS_EVENT, ProgressPayload::from(ev));
    };
    emit_one(ProgressEvent::Connected);

    let remote = adapter
        .fetch_remote_manifest(&profile)
        .map_err(|e| emit_error(&app, e.to_string()))?;
    let report = diff::compute(&local, remote.as_ref(), profile.prefer_diff);

    emit_one(ProgressEvent::DiffResolved {
        upload_count: report.upload.len(),
        upload_bytes: report.upload_bytes,
    });

    let plan = UploadPlan {
        profile: &profile,
        build_dir,
        local_manifest: local,
        diff: report,
    };

    let mut sink = EmitSink::new(app.clone());
    adapter
        .upload(&plan, &mut sink)
        .map_err(|e| emit_error(&app, e.to_string()))?;
    Ok(())
}

fn emit_error(app: &tauri::AppHandle, message: String) -> String {
    let _ = app.emit(PROGRESS_EVENT, ProgressPayload::Error { message: message.clone() });
    message
}

/// `ProgressSink`, der jedes Event als Tauri-Event emittiert.
struct EmitSink {
    app: tauri::AppHandle,
}

impl EmitSink {
    fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl ProgressSink for EmitSink {
    fn emit(&mut self, event: ProgressEvent) {
        let _ = self.app.emit(PROGRESS_EVENT, ProgressPayload::from(event));
    }
}

/// Build vor Deploy (Pflicht laut Decision-Doc), Profil und Secret laden.
fn ensure_built_and_load(
    state: &State<'_, AppState>,
    name: &str,
) -> Result<(DeployProfile, camino::Utf8PathBuf, Option<String>), String> {
    with_project(state, |project| {
        let profile = project
            .get_deploy_profile(name)
            .ok_or_else(|| format!("Profil `{name}` nicht gefunden"))?;
        profile.validate().map_err(to_str_err)?;

        // Build immer frisch — billiger als Risiko, dass alte Files
        // hochgeladen werden.
        sitebuilder::build_site(project).map_err(to_str_err)?;
        let build_dir = project.build_dir();

        let secret = crate::keystore::load_secret(project, &profile).map_err(to_str_err)?;
        if crate::keystore::requires_secret(&profile.auth) && secret.is_none() {
            return Err(format!(
                "Kein Secret im Keystore für Profil `{name}`. Bitte im Settings-Dialog setzen."
            ));
        }
        if !matches!(profile.auth, AuthMethod::Password { .. } | AuthMethod::SshKey { .. } | AuthMethod::GithubToken { .. }) {
            // exhaustive — Compiler-Sicherung, fängt zukünftige Varianten.
            return Err("Unbekannte Auth-Methode".into());
        }
        Ok((profile, build_dir, secret))
    })
}
