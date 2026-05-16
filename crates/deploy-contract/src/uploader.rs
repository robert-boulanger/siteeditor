//! Trait, gegen den konkrete Adapter (SFTP, GitHub-Pages, …)
//! implementieren. Bewusst **sync** im Trait — Adapter wrappen ihre
//! interne Async-Runtime (z.B. tokio für `russh`) selbst. Vorteil: der
//! Tauri-Layer muss nicht wissen, ob ein Target async ist, und Tests
//! brauchen kein Async-Setup.

use crate::diff::DiffReport;
use crate::manifest::Manifest;
use crate::profile::DeployProfile;
use camino::Utf8PathBuf;
use thiserror::Error;

/// Was die UI während eines Deploys angezeigt bekommen soll.
/// Wird über einen Channel (vom Caller bereitgestellt) emittiert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    /// Adapter hat sich verbunden / vorbereitet.
    Connected,
    /// Adapter hat das remote Manifest (oder dessen Fehlen) festgestellt.
    DiffResolved { upload_count: usize, upload_bytes: u64 },
    /// Eine Datei wird gerade hochgeladen.
    FileStart { rel_path: String, size: u64 },
    /// Datei fertig.
    FileDone { rel_path: String },
    /// Manifest wurde aktualisiert.
    ManifestWritten,
    /// Deploy abgeschlossen.
    Done { uploaded: usize, total_bytes: u64 },
}

/// Was der Caller dem Uploader übergibt: Build-Verzeichnis +
/// (optional vorab berechneter) Diff-Report.
pub struct UploadPlan<'a> {
    pub profile: &'a DeployProfile,
    pub build_dir: Utf8PathBuf,
    pub local_manifest: Manifest,
    pub diff: DiffReport,
}

#[derive(Debug, Error)]
pub enum UploaderError {
    #[error("Authentifizierung fehlgeschlagen: {0}")]
    Auth(String),
    #[error("Verbindung fehlgeschlagen: {0}")]
    Connect(String),
    #[error("Remote-I/O-Fehler: {0}")]
    Io(String),
    #[error("Lokaler I/O-Fehler: {0}")]
    Local(String),
    #[error("Profil ist invalide: {0}")]
    InvalidProfile(String),
    #[error("Abgebrochen")]
    Cancelled,
    #[error("Andere: {0}")]
    Other(String),
}

/// Wer Progress-Events empfangen will, implementiert das.
/// Default-Impl macht nichts (für headless Tests).
pub trait ProgressSink: Send {
    fn emit(&mut self, event: ProgressEvent);
}

impl ProgressSink for () {
    fn emit(&mut self, _event: ProgressEvent) {}
}

impl<F: FnMut(ProgressEvent) + Send> ProgressSink for F {
    fn emit(&mut self, event: ProgressEvent) {
        self(event);
    }
}

pub trait Uploader: Send {
    /// Liest das Remote-Manifest oder gibt `None` zurück, wenn keines da
    /// ist / nicht geparst werden kann. Adapter darf hier verbinden,
    /// muss aber nichts hochladen.
    fn fetch_remote_manifest(
        &mut self,
        profile: &DeployProfile,
    ) -> Result<Option<Manifest>, UploaderError>;

    /// Führt den eigentlichen Upload aus. Schreibt am Ende das neue
    /// Manifest auf Remote (`MANIFEST_FILENAME`).
    ///
    /// **Vertrag (Phase 10 §7):** Implementierungen MÜSSEN exakt die
    /// Files aus [`UploadPlan::diff`]`.upload` übertragen — nicht mehr
    /// (sonst geht die Diff-Ersparnis verloren), nicht weniger (sonst
    /// lügt die Dry-Run-Anzeige der UI). Das `plan.local_manifest` ist
    /// nur die Quelle für das Manifest-Write am Ende und für Bytegrößen
    /// in den `ProgressEvent`s.
    fn upload(
        &mut self,
        plan: &UploadPlan<'_>,
        progress: &mut dyn ProgressSink,
    ) -> Result<(), UploaderError>;
}
