//! `SftpAdapter` — konkrete Implementierung des [`Uploader`]-Traits.

use crate::path::{join_remote, normalize_rel_path};
use crate::runtime::AdapterRuntime;
use deploy_contract::{
    AuthMethod, DeployProfile, FileEntry, Manifest, ProgressEvent, ProgressSink, UploadPlan,
    Uploader, UploaderError, MANIFEST_FILENAME,
};
use russh::client::{Handle, Handler};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Adapter-Instanz. Hält das Secret (Passwort/Passphrase), das der
/// Tauri-Layer aus dem Keystore aufgelöst und an den Konstruktor
/// gereicht hat.
pub struct SftpAdapter {
    runtime: AdapterRuntime,
    secret: Option<String>,
}

impl SftpAdapter {
    /// `secret` = Passwort (bei [`AuthMethod::Password`]) oder
    /// SSH-Key-Passphrase (bei [`AuthMethod::SshKey`], optional). Für
    /// einen Key ohne Passphrase: `None`.
    pub fn new(secret: Option<String>) -> Self {
        Self {
            runtime: AdapterRuntime::new(),
            secret,
        }
    }
}

impl Uploader for SftpAdapter {
    fn fetch_remote_manifest(
        &mut self,
        profile: &DeployProfile,
    ) -> Result<Option<Manifest>, UploaderError> {
        let secret = self.secret.clone();
        self.runtime
            .get()
            .block_on(async move { fetch_remote_manifest_async(profile, secret.as_deref()).await })
    }

    fn upload(
        &mut self,
        plan: &UploadPlan<'_>,
        progress: &mut dyn ProgressSink,
    ) -> Result<(), UploaderError> {
        let secret = self.secret.clone();
        self.runtime.get().block_on(async {
            upload_async(plan, secret.as_deref(), progress).await
        })
    }
}

// --- Async-Implementierung ------------------------------------------------

async fn fetch_remote_manifest_async(
    profile: &DeployProfile,
    secret: Option<&str>,
) -> Result<Option<Manifest>, UploaderError> {
    let mut conn = SftpConnection::connect(profile, secret).await?;
    let manifest_path = join_remote(&profile.remote_path, MANIFEST_FILENAME);
    let bytes = match conn.read_file(&manifest_path).await {
        Ok(b) => b,
        Err(UploaderError::Io(_)) => return Ok(None), // i.d.R. „No such file"
        Err(other) => return Err(other),
    };
    match Manifest::from_json(&bytes) {
        Ok(m) => Ok(Some(m)),
        Err(_) => Ok(None), // beschädigt/inkompatibel → Caller fällt auf Full
    }
}

async fn upload_async(
    plan: &UploadPlan<'_>,
    secret: Option<&str>,
    progress: &mut dyn ProgressSink,
) -> Result<(), UploaderError> {
    let profile = plan.profile;
    let mut conn = SftpConnection::connect(profile, secret).await?;
    progress.emit(ProgressEvent::Connected);
    progress.emit(ProgressEvent::DiffResolved {
        upload_count: plan.diff.upload.len(),
        upload_bytes: plan.diff.upload_bytes,
    });

    // Welche Remote-Verzeichnisse müssen existieren, bevor wir Files
    // schreiben können? Sammeln + dedupen, dann erst anlegen.
    let mut required_dirs: BTreeSet<String> = BTreeSet::new();
    for rel in &plan.diff.upload {
        let rel_norm = normalize_rel_path(rel)
            .map_err(|e| UploaderError::InvalidProfile(e.to_string()))?;
        for ancestor in ancestor_dirs(&rel_norm) {
            required_dirs.insert(ancestor);
        }
    }
    for dir in required_dirs {
        let abs = join_remote(&profile.remote_path, &dir);
        conn.ensure_dir(&abs).await?;
    }

    // Sicherstellen, dass auch der remote_path selbst existiert (für
    // jungfräuliche Server). `ensure_dir` ist idempotent.
    conn.ensure_dir(profile.remote_path.trim_end_matches('/'))
        .await?;

    let mut uploaded = 0usize;
    let mut total_bytes = 0u64;
    for rel in &plan.diff.upload {
        let rel_norm = normalize_rel_path(rel)
            .map_err(|e| UploaderError::InvalidProfile(e.to_string()))?;
        let local = plan.build_dir.join(&rel_norm);
        let entry = plan.local_manifest.files.get(rel).cloned().unwrap_or(FileEntry {
            sha256: String::new(),
            size: 0,
        });
        progress.emit(ProgressEvent::FileStart {
            rel_path: rel_norm.clone(),
            size: entry.size,
        });

        let bytes = tokio::fs::read(local.as_std_path())
            .await
            .map_err(|e| UploaderError::Local(format!("{local}: {e}")))?;
        let remote = join_remote(&profile.remote_path, &rel_norm);
        conn.write_file(&remote, &bytes).await?;

        uploaded += 1;
        total_bytes += entry.size;
        progress.emit(ProgressEvent::FileDone { rel_path: rel_norm });
    }

    // Manifest am Ende schreiben — wenn etwas vorher schiefging, ist das
    // alte Manifest noch da und der nächste Deploy macht Full-Upload.
    let manifest_bytes = plan.local_manifest.to_json();
    let manifest_path = join_remote(&profile.remote_path, MANIFEST_FILENAME);
    conn.write_file(&manifest_path, &manifest_bytes).await?;
    progress.emit(ProgressEvent::ManifestWritten);

    progress.emit(ProgressEvent::Done {
        uploaded,
        total_bytes,
    });
    Ok(())
}

/// Liefert alle (relativen) Verzeichnis-Pfade, die für `rel` existieren
/// müssen. `a/b/c.txt` → `["a", "a/b"]`. Reihenfolge: flach → tief.
fn ancestor_dirs(rel: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parts: Vec<&str> = rel.split('/').collect();
    for i in 1..parts.len() {
        out.push(parts[..i].join("/"));
    }
    out
}

// --- russh-Wrapper --------------------------------------------------------

struct SftpConnection {
    sftp: SftpSession,
    // Wir halten den Session-Handle am Leben, damit der Channel nicht
    // unter uns geschlossen wird. `Handle` ist Cloneable.
    _session: Handle<HostKeyTrust>,
}

impl SftpConnection {
    async fn connect(
        profile: &DeployProfile,
        secret: Option<&str>,
    ) -> Result<Self, UploaderError> {
        let config = Arc::new(russh::client::Config::default());
        let handler = HostKeyTrust;
        let mut session =
            russh::client::connect(config, (profile.host.as_str(), profile.port), handler)
                .await
                .map_err(|e| UploaderError::Connect(e.to_string()))?;

        let authed = match &profile.auth {
            AuthMethod::Password { user } => {
                let pw = secret.ok_or_else(|| {
                    UploaderError::Auth("Passwort fehlt (kein Keystore-Eintrag)".into())
                })?;
                session
                    .authenticate_password(user, pw)
                    .await
                    .map_err(|e| UploaderError::Auth(e.to_string()))?
                    .success()
            }
            AuthMethod::SshKey { user, private_key_path } => {
                let key_bytes = tokio::fs::read(private_key_path).await.map_err(|e| {
                    UploaderError::Local(format!("SSH-Key {private_key_path}: {e}"))
                })?;
                let key_str = std::str::from_utf8(&key_bytes).map_err(|e| {
                    UploaderError::Local(format!("SSH-Key ist kein UTF-8: {e}"))
                })?;
                let key = russh::keys::decode_secret_key(key_str, secret)
                    .map_err(|e| UploaderError::Auth(format!("SSH-Key parsen: {e}")))?;
                let hash_alg = session.best_supported_rsa_hash().await
                    .map_err(|e| UploaderError::Auth(e.to_string()))?
                    .flatten();
                let pk = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
                session
                    .authenticate_publickey(user, pk)
                    .await
                    .map_err(|e| UploaderError::Auth(e.to_string()))?
                    .success()
            }
            AuthMethod::GithubToken { .. } => {
                return Err(UploaderError::InvalidProfile(
                    "SftpAdapter kann keine GithubToken-Auth verwenden".into(),
                ));
            }
        };
        if !authed {
            return Err(UploaderError::Auth("Authentifizierung abgelehnt".into()));
        }

        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| UploaderError::Connect(e.to_string()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| UploaderError::Connect(format!("SFTP-Subsystem: {e}")))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| UploaderError::Connect(format!("SFTP-Session: {e}")))?;

        Ok(Self {
            sftp,
            _session: session,
        })
    }

    async fn read_file(&mut self, remote_path: &str) -> Result<Vec<u8>, UploaderError> {
        let mut file = self
            .sftp
            .open(remote_path)
            .await
            .map_err(|e| UploaderError::Io(format!("open {remote_path}: {e}")))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .await
            .map_err(|e| UploaderError::Io(format!("read {remote_path}: {e}")))?;
        Ok(buf)
    }

    async fn write_file(&mut self, remote_path: &str, bytes: &[u8]) -> Result<(), UploaderError> {
        let mut file = self
            .sftp
            .open_with_flags(
                remote_path,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            )
            .await
            .map_err(|e| UploaderError::Io(format!("create {remote_path}: {e}")))?;
        file.write_all(bytes)
            .await
            .map_err(|e| UploaderError::Io(format!("write {remote_path}: {e}")))?;
        file.shutdown()
            .await
            .map_err(|e| UploaderError::Io(format!("close {remote_path}: {e}")))?;
        Ok(())
    }

    /// `mkdir`, aber idempotent: bestehende Verzeichnisse sind ok.
    async fn ensure_dir(&mut self, remote_path: &str) -> Result<(), UploaderError> {
        if remote_path.is_empty() || remote_path == "/" {
            return Ok(());
        }
        match self.sftp.metadata(remote_path).await {
            Ok(meta) if meta.is_dir() => return Ok(()),
            Ok(_) => {
                return Err(UploaderError::Io(format!(
                    "{remote_path} existiert, ist aber kein Verzeichnis"
                )))
            }
            Err(_) => {
                // Existiert nicht — Parent zuerst, dann mkdir.
                if let Some(idx) = remote_path.rfind('/') {
                    let parent = &remote_path[..idx];
                    if !parent.is_empty() {
                        Box::pin(self.ensure_dir(parent)).await?;
                    }
                }
                self.sftp
                    .create_dir(remote_path)
                    .await
                    .map_err(|e| UploaderError::Io(format!("mkdir {remote_path}: {e}")))?;
            }
        }
        Ok(())
    }
}

// --- Host-Key-Handler -----------------------------------------------------

/// Akzeptiert jeden Server-Key (TOFU-light). Persistente
/// `known_hosts`-Verifikation ist Phase 10.2 — UI muss diesen Trust-Status
/// klar kommunizieren.
#[derive(Clone, Copy)]
struct HostKeyTrust;

impl Handler for HostKeyTrust {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_dirs_flach_zu_tief() {
        assert!(ancestor_dirs("index.html").is_empty());
        assert_eq!(ancestor_dirs("a/b.txt"), vec!["a"]);
        assert_eq!(ancestor_dirs("a/b/c.txt"), vec!["a", "a/b"]);
        assert_eq!(
            ancestor_dirs("themes/default/styles/main.css"),
            vec!["themes", "themes/default", "themes/default/styles"]
        );
    }

    #[test]
    fn adapter_kann_konstruiert_werden_ohne_secret() {
        // Smoke: SftpAdapter::new soll nichts an einer Runtime-Vorbereitung
        // explodieren — runtime wird lazy gestartet, secret kann None sein.
        let _a = SftpAdapter::new(None);
        let _b = SftpAdapter::new(Some("hunter2".into()));
    }

    // Echter Connect-Path braucht einen Live-SSH-Server und wird
    // bewusst nicht im Unit-Test abgedeckt (siehe Decision-Doc §
    // „Tests parallel" — Live-Tests dokumentiert der User). Die
    // Defense-in-Depth-Validierung läuft eine Ebene tiefer:
    // [`DeployProfile::validate`] lehnt SFTP+GithubToken-Kombinationen
    // schon beim Speichern ab; dort sind die Tests.
}
