//! Hash-Manifest für den Diff-Upload. Wird nach jedem erfolgreichen
//! Deploy auf den Remote-Root geschrieben (`.siteeditor-manifest.json`)
//! und beim nächsten Deploy zurückgelesen, um die Diff-Liste zu bilden.
//!
//! Pfade im Manifest sind **relativ zum Build-Root** und nutzen
//! **Forward-Slashes** — egal auf welchem OS gebaut wurde (Windows-Pfade
//! werden beim Schreiben normalisiert). Das ist eine Public-API:
//! das Manifest landet beim Hoster, wir wollen kein Backslash-Chaos.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{self, Read};
use thiserror::Error;
use walkdir::WalkDir;

/// Schema-Version. Wenn das Format inkompatibel wechselt, hier hochzählen
/// — der Diff erkennt es und fällt automatisch auf Full-Upload zurück
/// (siehe [`crate::diff`]).
pub const MANIFEST_VERSION: &str = "1";

/// Dateiname auf dem Remote.
pub const MANIFEST_FILENAME: &str = ".siteeditor-manifest.json";

/// Ein Eintrag pro Datei. Größe getrennt, damit die UI „X Bytes werden
/// hochgeladen" anzeigen kann, ohne nochmal über das FS zu laufen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Konstant [`MANIFEST_VERSION`] zum Zeitpunkt des Schreibens.
    pub manifest_version: String,

    /// Identifier des erzeugenden Tools (für Debug/Forensik).
    #[serde(default = "default_built_by")]
    pub built_by: String,

    /// Sortierte Map relativer Pfade → Hash+Größe.
    /// BTreeMap → stabile Reihenfolge im JSON (wichtig für Diff &
    /// menschliche Lesbarkeit beim Debuggen).
    pub files: BTreeMap<String, FileEntry>,
}

fn default_built_by() -> String {
    format!("siteeditor/{}", env!("CARGO_PKG_VERSION"))
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            manifest_version: MANIFEST_VERSION.to_string(),
            built_by: default_built_by(),
            files: BTreeMap::new(),
        }
    }

    /// Manifest aus einem lokalen Build-Verzeichnis aufbauen.
    ///
    /// - Folgt Symlinks (Themes legen Assets als Symlinks ab; wir wollen
    ///   den Inhalt im Manifest, nicht den Link).
    /// - Ignoriert das Manifest selbst (falls schon eines liegt).
    /// - Pfade werden auf POSIX-Slashes normalisiert.
    pub fn from_directory(root: &Utf8Path) -> Result<Self, ManifestError> {
        let mut files = BTreeMap::new();
        for entry in WalkDir::new(root).follow_links(true) {
            let entry = entry.map_err(|e| ManifestError::Walk(e.to_string()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = entry.path();
            let rel = abs
                .strip_prefix(root.as_std_path())
                .map_err(|_| ManifestError::PathOutsideRoot(abs.display().to_string()))?;

            let rel_str = rel
                .to_str()
                .ok_or_else(|| ManifestError::NonUtf8Path(abs.display().to_string()))?
                .replace('\\', "/");

            if rel_str == MANIFEST_FILENAME {
                continue;
            }

            let (sha256, size) = hash_file(abs)?;
            files.insert(rel_str, FileEntry { sha256, size });
        }
        Ok(Self {
            manifest_version: MANIFEST_VERSION.to_string(),
            built_by: default_built_by(),
            files,
        })
    }

    /// Aus JSON-Bytes parsen. Akzeptiert nur die aktuelle
    /// `MANIFEST_VERSION` — andere Versionen geben `Err`, was den
    /// Caller (Diff) zwingt, auf Full-Upload zu fallen.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ManifestError> {
        let parsed: Manifest =
            serde_json::from_slice(bytes).map_err(|e| ManifestError::Parse(e.to_string()))?;
        if parsed.manifest_version != MANIFEST_VERSION {
            return Err(ManifestError::IncompatibleVersion {
                expected: MANIFEST_VERSION.into(),
                actual: parsed.manifest_version,
            });
        }
        Ok(parsed)
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("Manifest serialisiert immer")
    }

    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|f| f.size).sum()
    }
}

fn hash_file(path: &std::path::Path) -> Result<(String, u64), ManifestError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| ManifestError::Io(format!("{}: {}", path.display(), e)))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = read_some(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    let digest = hasher.finalize();
    Ok((hex(&digest), total))
}

fn read_some(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize, ManifestError> {
    match file.read(buf) {
        Ok(n) => Ok(n),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(0),
        Err(e) => Err(ManifestError::Io(e.to_string())),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Verzeichnis-Walk fehlgeschlagen: {0}")]
    Walk(String),
    #[error("Pfad außerhalb des Roots: {0}")]
    PathOutsideRoot(String),
    #[error("Pfad ist nicht UTF-8: {0}")]
    NonUtf8Path(String),
    #[error("Manifest-JSON ungültig: {0}")]
    Parse(String),
    #[error("Manifest-Version nicht unterstützt: erwartet {expected}, gefunden {actual}")]
    IncompatibleVersion { expected: String, actual: String },
    #[error("I/O: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    fn write(p: &std::path::Path, content: &[u8]) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn empty_directory_gives_empty_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(tmp.path().into()).unwrap();
        let m = Manifest::from_directory(&root).unwrap();
        assert!(m.files.is_empty());
        assert_eq!(m.total_bytes(), 0);
    }

    #[test]
    fn hashes_und_groessen_sind_deterministisch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        write(&root.join("index.html"), b"<html/>");
        write(&root.join("styles/main.css"), b"body{}");

        let m = Manifest::from_directory(&Utf8PathBuf::from_path_buf(root.clone()).unwrap()).unwrap();
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.files["index.html"].size, 7);
        assert_eq!(
            m.files["index.html"].sha256,
            // sha256("<html/>") — verifiziert mit `echo -n '<html/>' | sha256sum`
            "8397912ada2760dca34d1adb644cf54fc5c8d05d0ad56b4a6f99096b03ac8431"
        );
        // POSIX-Slashes, auch wenn auf Windows gebaut:
        assert!(m.files.contains_key("styles/main.css"));
    }

    #[test]
    fn manifest_datei_wird_ignoriert() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        write(&root.join("a.txt"), b"a");
        write(&root.join(MANIFEST_FILENAME), b"{}"); // soll ignoriert werden
        let m = Manifest::from_directory(&Utf8PathBuf::from_path_buf(root).unwrap()).unwrap();
        assert_eq!(m.files.len(), 1);
        assert!(m.files.contains_key("a.txt"));
    }

    #[test]
    fn json_roundtrip() {
        let mut m = Manifest::empty();
        m.files.insert(
            "x.html".into(),
            FileEntry { sha256: "abc".into(), size: 3 },
        );
        let bytes = m.to_json();
        let back = Manifest::from_json(&bytes).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn fremde_version_wird_abgelehnt() {
        let bytes = br#"{"manifest_version":"99","built_by":"x","files":{}}"#;
        let err = Manifest::from_json(bytes).unwrap_err();
        assert!(matches!(err, ManifestError::IncompatibleVersion { .. }));
    }
}
