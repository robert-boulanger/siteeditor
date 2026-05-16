//! Pfad-Sanitization für SFTP-Uploads.
//!
//! Lokale Manifest-Pfade kommen schon mit Forward-Slashes
//! ([`deploy_contract::manifest`] normalisiert sie), aber bevor wir
//! daraus einen Remote-Pfad bauen, müssen wir noch ein paar Klassen
//! abwehren:
//! - absolute Pfade (`/etc/...` würde unter dem konfigurierten Root
//!   ausbrechen)
//! - `..`-Segmente (`a/../../etc/passwd`)
//! - Backslashes (Windows-Pfade, die irgendwie durchgerutscht sind)
//! - leere Segmente / führende/trailing Slashes
//!
//! Das ist **defensive Tiefen-Verteidigung**: der Manifest-Builder
//! sollte schon saubere Pfade liefern, aber wir wollen die Garantie
//! auch direkt vor dem Network-I/O sehen.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    #[error("Pfad ist leer")]
    Empty,
    #[error("Pfad ist absolut: `{0}`")]
    Absolute(String),
    #[error("Pfad enthält Backslash: `{0}`")]
    Backslash(String),
    #[error("Pfad enthält `..`-Segment: `{0}`")]
    ParentSegment(String),
    #[error("Pfad enthält `.`-Segment: `{0}`")]
    DotSegment(String),
    #[error("Pfad enthält leeres Segment (doppelter Slash): `{0}`")]
    EmptySegment(String),
}

/// Validiert einen relativen Upload-Pfad und gibt ihn normalisiert
/// (Forward-Slashes, kein führender Slash) zurück.
pub fn normalize_rel_path(rel: &str) -> Result<String, PathError> {
    if rel.is_empty() {
        return Err(PathError::Empty);
    }
    if rel.contains('\\') {
        return Err(PathError::Backslash(rel.into()));
    }
    if rel.starts_with('/') {
        return Err(PathError::Absolute(rel.into()));
    }
    for seg in rel.split('/') {
        match seg {
            "" => return Err(PathError::EmptySegment(rel.into())),
            "." => return Err(PathError::DotSegment(rel.into())),
            ".." => return Err(PathError::ParentSegment(rel.into())),
            _ => {}
        }
    }
    Ok(rel.to_string())
}

/// Joint einen Remote-Root mit einem (bereits validierten) relativen
/// Pfad. Stellt sicher, dass der Root nicht mit doppelten Slashes endet.
pub fn join_remote(root: &str, rel: &str) -> String {
    let root_trim = root.trim_end_matches('/');
    if root_trim.is_empty() {
        // Server-Root (`/`). Resultierender Pfad bleibt mit führendem `/`.
        return format!("/{rel}");
    }
    format!("{root_trim}/{rel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn akzeptiert_normale_pfade() {
        assert_eq!(normalize_rel_path("index.html").unwrap(), "index.html");
        assert_eq!(normalize_rel_path("styles/main.css").unwrap(), "styles/main.css");
        assert_eq!(normalize_rel_path("a/b/c/d.txt").unwrap(), "a/b/c/d.txt");
    }

    #[test]
    fn lehnt_unsichere_pfade_ab() {
        assert_eq!(normalize_rel_path("").unwrap_err(), PathError::Empty);
        assert!(matches!(
            normalize_rel_path("/etc/passwd").unwrap_err(),
            PathError::Absolute(_)
        ));
        assert!(matches!(
            normalize_rel_path("a\\b").unwrap_err(),
            PathError::Backslash(_)
        ));
        assert!(matches!(
            normalize_rel_path("a/../etc").unwrap_err(),
            PathError::ParentSegment(_)
        ));
        assert!(matches!(
            normalize_rel_path("a/./b").unwrap_err(),
            PathError::DotSegment(_)
        ));
        assert!(matches!(
            normalize_rel_path("a//b").unwrap_err(),
            PathError::EmptySegment(_)
        ));
    }

    #[test]
    fn join_remote_baut_korrekten_pfad() {
        assert_eq!(join_remote("/var/www/site", "index.html"), "/var/www/site/index.html");
        assert_eq!(join_remote("/var/www/site/", "index.html"), "/var/www/site/index.html");
        // Trailing-Slashes werden konsumiert
        assert_eq!(join_remote("/var/www/site//", "a/b"), "/var/www/site/a/b");
        // Server-Root wird zum führenden Slash
        assert_eq!(join_remote("/", "a"), "/a");
        assert_eq!(join_remote("", "a"), "/a");
    }
}
