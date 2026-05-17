//! Deploy-Profil-Schema. Wird in `site.json.deploy_profiles` als Array
//! gespeichert; Credentials liegen NIE hier, sondern im OS-Keystore.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unterstützte Ziel-Protokolle. Erweiterbar — siehe Phase-10-Decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Sftp,
    Ftp,
    GithubPages,
}

/// Wie sich der Adapter beim Ziel authentifiziert. Welche Variante gültig
/// ist, hängt am [`Protocol`] — die Validierung sitzt in
/// [`DeployProfile::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthMethod {
    /// SFTP mit Passwort. Secret = Passwort, liegt im Keystore.
    Password { user: String },
    /// SFTP mit SSH-Key (Pfad zum private key). Secret = optionale Key-Passphrase.
    SshKey { user: String, private_key_path: String },
    /// GitHub Personal Access Token. Secret = der PAT.
    GithubToken {
        /// GitHub-User für den Push-URL (kann vom Repo-Owner abweichen).
        user: String,
    },
}

impl AuthMethod {
    /// Username für den Keystore-Eintrag — eindeutig pro Auth-Variante.
    pub fn keystore_username(&self) -> String {
        match self {
            AuthMethod::Password { user } => format!("password:{user}"),
            AuthMethod::SshKey { user, .. } => format!("sshkey:{user}"),
            AuthMethod::GithubToken { user } => format!("github:{user}"),
        }
    }
}

/// Ein einzelnes Deployment-Ziel. Mehrere pro Projekt erlaubt (Staging,
/// Prod, …). `name` ist Pflicht und projektweit eindeutig.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployProfile {
    /// Stabiler, projektweit eindeutiger Slug — wird auch im Keystore
    /// als Teil des Service-Namens verwendet.
    pub name: String,

    pub protocol: Protocol,

    /// Hostname / DNS-Name. Für `GithubPages` typisch `github.com`.
    pub host: String,

    /// Port. SFTP-Default 22, GitHub-Pages braucht 443 (HTTPS-Push).
    pub port: u16,

    /// Auth-Methode + die nicht-geheimen Felder (user, key-pfad).
    pub auth: AuthMethod,

    /// Remote-Zielpfad bzw. Repo-Identifier.
    /// - `Sftp`        → absoluter Pfad (`/var/www/site/`).
    /// - `GithubPages` → `<owner>/<repo>`.
    pub remote_path: String,

    /// Branch — nur für `GithubPages`. SFTP ignoriert das Feld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,

    /// Diff-Upload bevorzugen (true) oder immer Full (false).
    /// Decision-Default: `true` (siehe Phase-10-Decision-Doc §7).
    #[serde(default = "default_diff")]
    pub prefer_diff: bool,
}

fn default_diff() -> bool {
    true
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("Profil-Name darf nicht leer sein")]
    EmptyName,
    #[error("Profil-Name enthält ungültige Zeichen: `{0}` (erlaubt: A-Z, a-z, 0-9, _, -)")]
    InvalidName(String),
    #[error("Profil-Name zu lang ({0} Zeichen, max 64)")]
    NameTooLong(usize),
    #[error("Host darf nicht leer sein")]
    EmptyHost,
    #[error("Remote-Pfad darf nicht leer sein")]
    EmptyRemotePath,
    #[error("SFTP-Profile dürfen keine GithubToken-Auth verwenden")]
    AuthMismatchSftpExpectsSshOrPassword,
    #[error("FTP-Profile dürfen ausschließlich Password-Auth verwenden")]
    AuthMismatchFtpExpectsPassword,
    #[error("GitHub-Pages-Profile dürfen ausschließlich GithubToken-Auth verwenden")]
    AuthMismatchGithubExpectsToken,
    #[error("GitHub-Pages: `remote_path` muss `<owner>/<repo>` sein, war `{0}`")]
    GithubRepoMalformed(String),
}

impl DeployProfile {
    /// Validiert das Profil. Wird beim Speichern und vor jedem Deploy
    /// aufgerufen — UI darf sich darauf verlassen, dass ein erfolgreich
    /// gespeichertes Profil hier wieder durchkommt.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.is_empty() {
            return Err(ProfileError::EmptyName);
        }
        if self.name.len() > 64 {
            return Err(ProfileError::NameTooLong(self.name.len()));
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ProfileError::InvalidName(self.name.clone()));
        }
        if self.host.is_empty() {
            return Err(ProfileError::EmptyHost);
        }
        if self.remote_path.is_empty() {
            return Err(ProfileError::EmptyRemotePath);
        }
        match (self.protocol, &self.auth) {
            (Protocol::Sftp, AuthMethod::GithubToken { .. }) => {
                return Err(ProfileError::AuthMismatchSftpExpectsSshOrPassword);
            }
            (Protocol::Ftp, AuthMethod::SshKey { .. })
            | (Protocol::Ftp, AuthMethod::GithubToken { .. }) => {
                return Err(ProfileError::AuthMismatchFtpExpectsPassword);
            }
            (Protocol::GithubPages, AuthMethod::Password { .. })
            | (Protocol::GithubPages, AuthMethod::SshKey { .. }) => {
                return Err(ProfileError::AuthMismatchGithubExpectsToken);
            }
            _ => {}
        }
        if self.protocol == Protocol::GithubPages {
            // Erwartung: `owner/repo` — genau ein `/`, beide Hälften nicht leer.
            let parts: Vec<&str> = self.remote_path.split('/').collect();
            if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                return Err(ProfileError::GithubRepoMalformed(self.remote_path.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sftp_password() -> DeployProfile {
        DeployProfile {
            name: "prod".into(),
            protocol: Protocol::Sftp,
            host: "example.com".into(),
            port: 22,
            auth: AuthMethod::Password { user: "deploy".into() },
            remote_path: "/var/www/site".into(),
            branch: None,
            prefer_diff: true,
        }
    }

    fn github_pages() -> DeployProfile {
        DeployProfile {
            name: "ghpages".into(),
            protocol: Protocol::GithubPages,
            host: "github.com".into(),
            port: 443,
            auth: AuthMethod::GithubToken { user: "octocat".into() },
            remote_path: "octocat/mysite".into(),
            branch: Some("gh-pages".into()),
            prefer_diff: true,
        }
    }

    #[test]
    fn gueltige_profile_validieren() {
        sftp_password().validate().unwrap();
        github_pages().validate().unwrap();
    }

    #[test]
    fn leerer_name_invalid() {
        let mut p = sftp_password();
        p.name = "".into();
        assert!(matches!(p.validate(), Err(ProfileError::EmptyName)));
    }

    #[test]
    fn name_mit_sonderzeichen_invalid() {
        let mut p = sftp_password();
        p.name = "prod/staging".into();
        assert!(matches!(p.validate(), Err(ProfileError::InvalidName(_))));
    }

    #[test]
    fn sftp_mit_token_invalid() {
        let mut p = sftp_password();
        p.auth = AuthMethod::GithubToken { user: "x".into() };
        assert!(matches!(p.validate(), Err(ProfileError::AuthMismatchSftpExpectsSshOrPassword)));
    }

    #[test]
    fn github_mit_password_invalid() {
        let mut p = github_pages();
        p.auth = AuthMethod::Password { user: "x".into() };
        assert!(matches!(p.validate(), Err(ProfileError::AuthMismatchGithubExpectsToken)));
    }

    #[test]
    fn github_repo_braucht_owner_und_repo() {
        let mut p = github_pages();
        p.remote_path = "no-slash".into();
        assert!(matches!(p.validate(), Err(ProfileError::GithubRepoMalformed(_))));

        p.remote_path = "owner/".into();
        assert!(matches!(p.validate(), Err(ProfileError::GithubRepoMalformed(_))));
    }

    #[test]
    fn serde_roundtrip_inklusive_default_diff() {
        let p = sftp_password();
        let json = serde_json::to_string(&p).unwrap();
        let back: DeployProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);

        // Default für prefer_diff greift, wenn das Feld im JSON fehlt:
        let minimal = r#"{
            "name":"p","protocol":"sftp","host":"x","port":22,
            "auth":{"kind":"password","user":"u"},
            "remote_path":"/x"
        }"#;
        let p: DeployProfile = serde_json::from_str(minimal).unwrap();
        assert!(p.prefer_diff);
    }

    fn ftp_password() -> DeployProfile {
        DeployProfile {
            name: "prod-ftp".into(),
            protocol: Protocol::Ftp,
            host: "srv05.nanet.at".into(),
            port: 21,
            auth: AuthMethod::Password { user: "d001704elisabeth".into() },
            remote_path: "/htdocs".into(),
            branch: None,
            prefer_diff: true,
        }
    }

    #[test]
    fn ftp_mit_password_valid() {
        ftp_password().validate().unwrap();
    }

    #[test]
    fn ftp_mit_sshkey_invalid() {
        let mut p = ftp_password();
        p.auth = AuthMethod::SshKey { user: "u".into(), private_key_path: "/k".into() };
        assert!(matches!(p.validate(), Err(ProfileError::AuthMismatchFtpExpectsPassword)));
    }

    #[test]
    fn ftp_mit_token_invalid() {
        let mut p = ftp_password();
        p.auth = AuthMethod::GithubToken { user: "x".into() };
        assert!(matches!(p.validate(), Err(ProfileError::AuthMismatchFtpExpectsPassword)));
    }

    #[test]
    fn ftp_serde_roundtrip() {
        let p = ftp_password();
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"protocol\":\"ftp\""));
        let back: DeployProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn keystore_username_eindeutig_pro_auth_typ() {
        assert_eq!(
            AuthMethod::Password { user: "u".into() }.keystore_username(),
            "password:u"
        );
        assert_eq!(
            AuthMethod::SshKey { user: "u".into(), private_key_path: "/k".into() }.keystore_username(),
            "sshkey:u"
        );
        assert_eq!(
            AuthMethod::GithubToken { user: "u".into() }.keystore_username(),
            "github:u"
        );
    }
}
