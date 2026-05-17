//! `FtpAdapter` — plain FTP, Passive-Mode, UTF-8.
//!
//! Verwendet die **Sync-API** von `suppaftp` (`FtpStream`). Tauri ruft
//! Commands ohnehin auf einem Blocking-Thread auf, daher kein eigener
//! async-Runtime und kein doppeltes Async-Modell wie im SFTP-Adapter.

use crate::path::{ancestor_dirs, join_remote, normalize_rel_path};
use deploy_contract::{
    AuthMethod, DeployProfile, FileEntry, Manifest, ProgressEvent, ProgressSink, UploadPlan,
    Uploader, UploaderError, MANIFEST_FILENAME,
};
use std::collections::BTreeSet;
use std::io::Cursor;
use suppaftp::types::FileType;
use suppaftp::{FtpError, FtpStream, Mode, Status};

pub struct FtpAdapter {
    secret: Option<String>,
}

impl FtpAdapter {
    pub fn new(secret: Option<String>) -> Self {
        Self { secret }
    }
}

impl Uploader for FtpAdapter {
    fn fetch_remote_manifest(
        &mut self,
        profile: &DeployProfile,
    ) -> Result<Option<Manifest>, UploaderError> {
        let mut conn = connect(profile, self.secret.as_deref())?;
        let manifest_path = join_remote(&profile.remote_path, MANIFEST_FILENAME);
        let bytes = match conn.retr_as_buffer(&manifest_path) {
            Ok(buf) => buf.into_inner(),
            Err(_) => {
                let _ = conn.quit();
                return Ok(None); // i.d.R. „550 No such file"
            }
        };
        let _ = conn.quit();
        match Manifest::from_json(&bytes) {
            Ok(m) => Ok(Some(m)),
            Err(_) => Ok(None),
        }
    }

    fn upload(
        &mut self,
        plan: &UploadPlan<'_>,
        progress: &mut dyn ProgressSink,
    ) -> Result<(), UploaderError> {
        let profile = plan.profile;
        let mut conn = connect(profile, self.secret.as_deref())?;
        progress.emit(ProgressEvent::Connected);
        progress.emit(ProgressEvent::DiffResolved {
            upload_count: plan.diff.upload.len(),
            upload_bytes: plan.diff.upload_bytes,
        });

        // remote_path selbst sicherstellen (jungfräulicher Server).
        ensure_dir(&mut conn, profile.remote_path.trim_end_matches('/'))?;

        // Alle Parent-Dirs sammeln + dedupen, dann erst anlegen.
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
            ensure_dir(&mut conn, &abs)?;
        }

        let mut uploaded = 0usize;
        let mut total_bytes = 0u64;
        for rel in &plan.diff.upload {
            let rel_norm = normalize_rel_path(rel)
                .map_err(|e| UploaderError::InvalidProfile(e.to_string()))?;
            let local = plan.build_dir.join(&rel_norm);
            let entry = plan
                .local_manifest
                .files
                .get(rel)
                .cloned()
                .unwrap_or(FileEntry { sha256: String::new(), size: 0 });
            progress.emit(ProgressEvent::FileStart {
                rel_path: rel_norm.clone(),
                size: entry.size,
            });

            let bytes = std::fs::read(local.as_std_path())
                .map_err(|e| UploaderError::Local(format!("{local}: {e}")))?;
            let remote = join_remote(&profile.remote_path, &rel_norm);
            let mut cursor = Cursor::new(bytes);
            conn.put_file(&remote, &mut cursor)
                .map_err(|e| UploaderError::Io(format!("STOR {remote}: {e}")))?;

            uploaded += 1;
            total_bytes += entry.size;
            progress.emit(ProgressEvent::FileDone { rel_path: rel_norm });
        }

        // Manifest am Ende — wenn vorher etwas schiefging, bleibt das
        // alte Manifest stehen und der nächste Deploy macht Full.
        let manifest_bytes = plan.local_manifest.to_json();
        let manifest_path = join_remote(&profile.remote_path, MANIFEST_FILENAME);
        let mut cursor = Cursor::new(manifest_bytes);
        conn.put_file(&manifest_path, &mut cursor)
            .map_err(|e| UploaderError::Io(format!("STOR {manifest_path}: {e}")))?;
        progress.emit(ProgressEvent::ManifestWritten);

        let _ = conn.quit();
        progress.emit(ProgressEvent::Done { uploaded, total_bytes });
        Ok(())
    }
}

fn connect(profile: &DeployProfile, secret: Option<&str>) -> Result<FtpStream, UploaderError> {
    let (user, password) = match &profile.auth {
        AuthMethod::Password { user } => {
            let pw = secret.ok_or_else(|| {
                UploaderError::Auth("Passwort fehlt (kein Keystore-Eintrag)".into())
            })?;
            (user.as_str(), pw)
        }
        AuthMethod::SshKey { .. } | AuthMethod::GithubToken { .. } => {
            return Err(UploaderError::InvalidProfile(
                "FtpAdapter erwartet AuthMethod::Password".into(),
            ));
        }
    };

    let addr = format!("{}:{}", profile.host, profile.port);
    let mut ftp = FtpStream::connect(&addr)
        .map_err(|e| UploaderError::Connect(format!("{addr}: {e}")))?;
    // PASV (RFC 959) kennt nur IPv4 — bei IPv6-Verbindungen antwortet
    // der Server zwar mit „227 Entering passive mode (::1,…)", aber
    // suppaftp lehnt das als invalid response ab. RFC 2428 EPSV löst
    // das. Wir schalten automatisch um, sobald die Control-Connection
    // IPv6 ist; für IPv4 bleibt PASV (kompatibel mit NAT-Hostern).
    if matches!(
        ftp.get_ref().peer_addr().ok(),
        Some(std::net::SocketAddr::V6(_))
    ) {
        ftp.set_mode(Mode::ExtendedPassive);
    }
    ftp.login(user, password)
        .map_err(|e| UploaderError::Auth(e.to_string()))?;
    ftp.transfer_type(FileType::Binary)
        .map_err(|e| UploaderError::Connect(format!("TYPE I: {e}")))?;
    // UTF-8: best-effort. Server, der das nicht kennt, antwortet mit
    // einer 5xx; in dem Fall ignorieren wir und hoffen auf ASCII-Pfade.
    let _ = ftp.site("UTF8 ON");
    Ok(ftp)
}

/// `MKD` mit parent-first Rekursion. „existiert schon" (typisch 550 /
/// 521) wird geschluckt, alles andere bubbelt hoch.
fn ensure_dir(conn: &mut FtpStream, remote_path: &str) -> Result<(), UploaderError> {
    if remote_path.is_empty() || remote_path == "/" {
        return Ok(());
    }
    // Wenn CWD klappt, existiert das Verzeichnis bereits.
    if conn.cwd(remote_path).is_ok() {
        // Zurück nach „/" damit nachfolgende absolute Pfade nicht relativ
        // interpretiert werden (suppaftp lässt das eigentlich absolut,
        // aber sicher ist sicher).
        let _ = conn.cwd("/");
        return Ok(());
    }
    // Parent zuerst.
    if let Some(idx) = remote_path.rfind('/') {
        let parent = &remote_path[..idx];
        if !parent.is_empty() {
            ensure_dir(conn, parent)?;
        }
    }
    match conn.mkdir(remote_path) {
        Ok(_) => Ok(()),
        Err(FtpError::UnexpectedResponse(resp))
            if matches!(
                resp.status,
                Status::FileUnavailable | Status::BadFilename
            ) =>
        {
            // Race: zwischen CWD und MKD hat jemand das Dir angelegt.
            // (Oder Server-Quirk.) Wir interpretieren als „existiert".
            Ok(())
        }
        Err(e) => Err(UploaderError::Io(format!("MKD {remote_path}: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_kann_konstruiert_werden() {
        let _a = FtpAdapter::new(None);
        let _b = FtpAdapter::new(Some("pw".into()));
    }
}
