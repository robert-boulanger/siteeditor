//! Diff-Logik: was muss hochgeladen werden?
//!
//! Pure-Function-Layer, kein I/O. Caller (Tauri-Command oder Adapter)
//! liefert lokales und remote Manifest; wir geben das Upload-Set zurück.
//!
//! Decision (Phase-10 §7): bei jedem Fall, wo das Remote-Manifest fehlt
//! oder inkompatibel ist, fallen wir auf **Full-Upload** zurück. „Diff
//! verlässlich funktioniert" heißt: lokales und remote Manifest sind
//! beide gültig.

use crate::manifest::Manifest;
use std::collections::BTreeSet;

/// Welche Strategie wurde gewählt, und warum?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStrategy {
    /// Inkrementell: nur die Diff-Files hochladen.
    Incremental,
    /// Komplett: alle Files hochladen.
    /// `reason` ist eine kurze, UI-taugliche Begründung.
    Full { reason: String },
}

/// Report, der UI und Adapter direkt nutzen können.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffReport {
    pub strategy: DiffStrategy,

    /// Relative POSIX-Pfade, die übertragen werden müssen.
    /// Bei [`DiffStrategy::Full`] enthält das alle lokalen Files.
    pub upload: Vec<String>,

    /// Relative Pfade, die remote existieren, lokal aber nicht mehr.
    /// **Phase 10 löscht nichts** — Liste ist informativ für die UI
    /// („Diese Files könnten obsolet sein"). Remote-Cleanup ist Phase 10.1.
    pub orphan_remote: Vec<String>,

    /// Bytes, die hochgeladen werden müssen (Summe über `upload`).
    pub upload_bytes: u64,
}

/// Vergleicht lokal und (optionales) Remote-Manifest und entscheidet
/// die Strategie. `prefer_diff=false` zwingt auf Full-Upload.
pub fn compute(
    local: &Manifest,
    remote: Option<&Manifest>,
    prefer_diff: bool,
) -> DiffReport {
    if !prefer_diff {
        return full_report(local, "User-Profil bevorzugt Full-Upload");
    }
    let Some(remote) = remote else {
        return full_report(local, "Kein Remote-Manifest gefunden (Erst-Deploy)");
    };

    // Diff-Pfad. Manifest-Version wurde beim Parsen schon geprüft;
    // wenn wir hier sind, sind beide Manifeste gültig und kompatibel.
    let local_keys: BTreeSet<&String> = local.files.keys().collect();
    let remote_keys: BTreeSet<&String> = remote.files.keys().collect();

    let mut upload: Vec<String> = Vec::new();
    let mut upload_bytes: u64 = 0;
    for key in &local_keys {
        let local_entry = &local.files[*key];
        let needs_upload = match remote.files.get(*key) {
            None => true,
            Some(remote_entry) => remote_entry.sha256 != local_entry.sha256,
        };
        if needs_upload {
            upload.push((*key).clone());
            upload_bytes += local_entry.size;
        }
    }
    upload.sort();

    let orphan_remote: Vec<String> = remote_keys
        .difference(&local_keys)
        .map(|s| (*s).clone())
        .collect();

    DiffReport {
        strategy: DiffStrategy::Incremental,
        upload,
        orphan_remote,
        upload_bytes,
    }
}

fn full_report(local: &Manifest, reason: &str) -> DiffReport {
    let mut upload: Vec<String> = local.files.keys().cloned().collect();
    upload.sort();
    DiffReport {
        strategy: DiffStrategy::Full { reason: reason.into() },
        upload,
        orphan_remote: Vec::new(),
        upload_bytes: local.total_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::FileEntry;

    fn m(entries: &[(&str, &str, u64)]) -> Manifest {
        let mut m = Manifest::empty();
        for (path, sha, size) in entries {
            m.files.insert(
                (*path).into(),
                FileEntry { sha256: (*sha).into(), size: *size },
            );
        }
        m
    }

    #[test]
    fn fehlendes_remote_manifest_fuehrt_zu_full() {
        let local = m(&[("a.html", "h1", 10), ("b.css", "h2", 5)]);
        let r = compute(&local, None, true);
        assert!(matches!(r.strategy, DiffStrategy::Full { .. }));
        assert_eq!(r.upload, vec!["a.html", "b.css"]);
        assert_eq!(r.upload_bytes, 15);
    }

    #[test]
    fn prefer_diff_false_erzwingt_full_auch_bei_remote() {
        let local = m(&[("a", "h1", 1)]);
        let remote = m(&[("a", "h1", 1)]);
        let r = compute(&local, Some(&remote), false);
        assert!(matches!(r.strategy, DiffStrategy::Full { .. }));
        assert_eq!(r.upload, vec!["a"]);
    }

    #[test]
    fn identische_manifeste_brauchen_keinen_upload() {
        let local = m(&[("a", "h1", 1), ("b", "h2", 2)]);
        let r = compute(&local, Some(&local.clone()), true);
        assert_eq!(r.strategy, DiffStrategy::Incremental);
        assert!(r.upload.is_empty());
        assert_eq!(r.upload_bytes, 0);
        assert!(r.orphan_remote.is_empty());
    }

    #[test]
    fn neue_und_geaenderte_files_landen_im_upload() {
        let local = m(&[
            ("kept.html", "h-old", 1),
            ("changed.html", "h-new", 2),
            ("added.css", "h-add", 3),
        ]);
        let remote = m(&[
            ("kept.html", "h-old", 1),
            ("changed.html", "h-different", 2),
            ("gone.png", "h-gone", 99),
        ]);
        let r = compute(&local, Some(&remote), true);
        assert_eq!(r.strategy, DiffStrategy::Incremental);
        assert_eq!(r.upload, vec!["added.css", "changed.html"]);
        assert_eq!(r.upload_bytes, 5); // 2 + 3
        assert_eq!(r.orphan_remote, vec!["gone.png"]);
    }
}
