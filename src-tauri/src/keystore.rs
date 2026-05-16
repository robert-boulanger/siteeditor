//! Cross-platform Wrapper um `keyring` (macOS Keychain / Windows
//! Credential Manager / Linux Secret Service).
//!
//! Wir speichern pro Profil **ein** Secret: Passwort, SSH-Key-Passphrase,
//! oder GitHub-PAT — abhängig vom Profil-Typ. Welcher Service- und
//! Username-Schlüssel verwendet wird, kommt aus [`Project::keystore_service_for`]
//! und [`AuthMethod::keystore_username`], damit Adapter und Tauri-Layer
//! exakt dieselben Keys nutzen.

use deploy_contract::{AuthMethod, DeployProfile};
use keyring::Entry;
use projectfs::Project;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("Keystore-I/O: {0}")]
    Backend(String),
    #[error("Kein Eintrag gefunden")]
    NotFound,
}

impl From<keyring::Error> for KeystoreError {
    fn from(e: keyring::Error) -> Self {
        match e {
            keyring::Error::NoEntry => KeystoreError::NotFound,
            other => KeystoreError::Backend(other.to_string()),
        }
    }
}

fn entry(project: &Project, profile: &DeployProfile) -> Result<Entry, KeystoreError> {
    let service = project.keystore_service_for(&profile.name);
    let user = profile.auth.keystore_username();
    Entry::new(&service, &user).map_err(KeystoreError::from)
}

/// Schreibt das Secret. Überschreibt einen ggf. existierenden Eintrag.
pub fn store_secret(
    project: &Project,
    profile: &DeployProfile,
    secret: &str,
) -> Result<(), KeystoreError> {
    let e = entry(project, profile)?;
    e.set_password(secret).map_err(KeystoreError::from)
}

/// Liest das Secret. `Ok(None)` wenn es keinen Eintrag gibt — Adapter
/// kann dann z.B. SSH-Agent versuchen oder den User um Eingabe bitten.
pub fn load_secret(
    project: &Project,
    profile: &DeployProfile,
) -> Result<Option<String>, KeystoreError> {
    let e = entry(project, profile)?;
    match e.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(other) => Err(KeystoreError::Backend(other.to_string())),
    }
}

/// Löscht das Secret. Kein Fehler, wenn keines existiert.
pub fn delete_secret(project: &Project, profile: &DeployProfile) -> Result<(), KeystoreError> {
    let e = entry(project, profile)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(other) => Err(KeystoreError::Backend(other.to_string())),
    }
}

/// Pflicht-Check pro Auth-Typ: ist ein Secret nötig, bevor wir
/// einen Deploy starten können?
pub fn requires_secret(auth: &AuthMethod) -> bool {
    match auth {
        AuthMethod::Password { .. } => true,
        AuthMethod::SshKey { .. } => false, // Passphrase optional
        AuthMethod::GithubToken { .. } => true,
    }
}
