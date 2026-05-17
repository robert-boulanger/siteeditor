//! FTP-Adapter für Phase 10 (plain FTP, Passive-Mode, UTF-8).
//!
//! Pendant zum SFTP-Adapter, implementiert denselben
//! [`deploy_contract::Uploader`]-Trait. Kein Vertragsbruch — Diff- und
//! Manifest-Logik unverändert.
//!
//! ## Designentscheidungen
//!
//! - **suppaftp Sync-API.** Kein eigener async-Runtime, Tauri ruft die
//!   Commands ohnehin auf einem Blocking-Thread auf.
//! - **Pro Deploy eine frische Verbindung.** Kein Pool — gleiche Linie
//!   wie [`deploy_sftp`].
//! - **Plain FTP only.** FTPS (AUTH TLS) ist als Profil-Schalter
//!   vorgesehen, in dieser Iteration aber nicht verdrahtet.

mod adapter;
mod path;

pub use adapter::FtpAdapter;
pub use path::{normalize_rel_path, PathError};
