//! Lazily-konstruierter Single-Thread-Tokio-Runtime. Eine Instanz pro
//! `SftpAdapter`, gehalten in einem `OnceCell` damit `block_on` aus
//! mehreren Methoden auf derselben Runtime landet.

use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};

pub(crate) struct AdapterRuntime {
    inner: OnceLock<Runtime>,
}

impl AdapterRuntime {
    pub fn new() -> Self {
        Self { inner: OnceLock::new() }
    }

    pub fn get(&self) -> &Runtime {
        self.inner.get_or_init(|| {
            // Single-thread reicht: ein SFTP-Deploy ist sequenziell I/O,
            // kein CPU-Pool nötig. Spart Threads & Startkosten.
            Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime startbar")
        })
    }
}
