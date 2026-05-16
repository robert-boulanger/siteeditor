//! SFTP-Adapter für Phase 10. Implementiert [`deploy_contract::Uploader`]
//! gegen [`russh`] + [`russh-sftp`].
//!
//! ## Designentscheidungen
//!
//! - **Sync-Trait, intern async.** Wir besitzen einen eigenen
//!   [`tokio::runtime::Runtime`] und brücken jeden Trait-Call mit
//!   `runtime.block_on(...)`. Damit muss der Tauri-Layer nichts über
//!   Async wissen — und Tests bleiben einfach.
//! - **Pro Deploy eine frische Connection.** Kein Pool, kein
//!   shared-state — Phase-10-Decision §1.
//! - **Path-Traversal-Schutz.** Jeder Upload-Pfad wird auf Forward-Slashes
//!   normalisiert und gegen `..`-Segmente gecheckt, bevor wir an die
//!   SFTP-Schicht übergeben.
//! - **Host-Key-Trust v1: TOFU-light.** Wir akzeptieren beim ersten
//!   Connect jeden Server-Key (siehe [`AcceptAnyHostKey`]). Persistente
//!   `known_hosts`-Verifikation ist eine eigene kleine Phase 10.2; ohne
//!   sie würde Phase 10 für den User schon am ersten Connect scheitern.
//!   Der Status-Text muss diesen Trade-off klar machen.

mod adapter;
mod path;
mod runtime;

pub use adapter::SftpAdapter;
pub use path::{normalize_rel_path, PathError};
