//! Vertrag für Phase 10 — Deployment.
//!
//! Diese Crate definiert das gemeinsame Schema (Profile, Manifest, Diff)
//! und den `Uploader`-Trait, gegen den die konkreten Adapter
//! (`deploy-sftp`, `deploy-github-pages`, …) implementieren. Sie hängt
//! nicht von `tauri` oder einem konkreten Transport ab — pure Logik +
//! Filesystem-Hashing, getestet ohne Netzwerk.
//!
//! ## Module
//! - [`profile`]  — Konfigurations-Schema des [`DeployProfile`].
//! - [`manifest`] — Hash-Manifest fürs Diff-Upload.
//! - [`diff`]     — Vergleich lokal vs. remote → Upload-Set.
//! - [`uploader`] — async-frei vorerst (sync-Trait + `Box<dyn Error>`);
//!   konkrete Adapter wrappen ihre Async-Runtime intern.

pub mod diff;
pub mod manifest;
pub mod profile;
pub mod uploader;

pub use diff::{DiffReport, DiffStrategy};
pub use manifest::{FileEntry, Manifest, MANIFEST_FILENAME, MANIFEST_VERSION};
pub use profile::{AuthMethod, DeployProfile, Protocol};
pub use uploader::{ProgressEvent, ProgressSink, UploadPlan, Uploader, UploaderError};
