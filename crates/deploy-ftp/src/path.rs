//! Pfad-Sanitization für FTP-Uploads. Bit-identische Kopie der
//! SFTP-Variante — wir wollen die Garantien direkt vor dem Network-I/O
//! sehen, unabhängig vom Transport. Refactor zu einem geteilten Modul
//! kommt in eigenem Commit (siehe Plan).

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

pub fn join_remote(root: &str, rel: &str) -> String {
    let root_trim = root.trim_end_matches('/');
    if root_trim.is_empty() {
        return format!("/{rel}");
    }
    format!("{root_trim}/{rel}")
}

pub fn ancestor_dirs(rel: &str) -> Vec<String> {
    let mut out = Vec::new();
    let parts: Vec<&str> = rel.split('/').collect();
    for i in 1..parts.len() {
        out.push(parts[..i].join("/"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn akzeptiert_normale_pfade() {
        assert_eq!(normalize_rel_path("index.html").unwrap(), "index.html");
        assert_eq!(normalize_rel_path("a/b/c.txt").unwrap(), "a/b/c.txt");
    }

    #[test]
    fn lehnt_unsichere_pfade_ab() {
        assert_eq!(normalize_rel_path("").unwrap_err(), PathError::Empty);
        assert!(matches!(normalize_rel_path("/etc").unwrap_err(), PathError::Absolute(_)));
        assert!(matches!(normalize_rel_path("a\\b").unwrap_err(), PathError::Backslash(_)));
        assert!(matches!(normalize_rel_path("a/../b").unwrap_err(), PathError::ParentSegment(_)));
        assert!(matches!(normalize_rel_path("a/./b").unwrap_err(), PathError::DotSegment(_)));
        assert!(matches!(normalize_rel_path("a//b").unwrap_err(), PathError::EmptySegment(_)));
    }

    #[test]
    fn join_remote_baut_korrekten_pfad() {
        assert_eq!(join_remote("/htdocs", "i.html"), "/htdocs/i.html");
        assert_eq!(join_remote("/htdocs/", "a/b"), "/htdocs/a/b");
        assert_eq!(join_remote("", "a"), "/a");
        assert_eq!(join_remote("/", "a"), "/a");
    }

    #[test]
    fn ancestor_dirs_flach_zu_tief() {
        assert!(ancestor_dirs("i.html").is_empty());
        assert_eq!(ancestor_dirs("a/b.txt"), vec!["a"]);
        assert_eq!(ancestor_dirs("a/b/c.txt"), vec!["a", "a/b"]);
    }
}
