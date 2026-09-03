use std::path::Path;
use std::sync::Arc;

use made_core::error::DomainError;

use crate::engine::detect::{engine_of, StorageEngine};
use crate::engine::redb::RedbEngine;
use crate::engine::Engine;

use super::RedbCeremonyStore;

impl RedbCeremonyStore {
    /// Open, creating the database and its tables when absent.
    /// Open the store at `path`, on whichever engine wrote it.
    ///
    /// An existing store is opened by the engine its own bytes name, so it
    /// can never be opened by the wrong one. A path with no store yet gets
    /// redb, the default.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with(path, None)
    }

    /// Open on the WAL-mode SQLite engine, which several processes can hold
    /// at once. Only available when built with the `sqlite` feature.
    #[cfg(feature = "sqlite")]
    pub fn open_sqlite(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with(path, Some(StorageEngine::Sqlite))
    }

    /// [`open`](Self::open) with a say in the engine.
    ///
    /// `wanted` decides only what a **new** store becomes. An existing one
    /// always opens on the engine that wrote it, and asking for a different
    /// engine is refused by name rather than silently ignored — a store
    /// opened as the wrong engine is how a user ends up with two half-full
    /// files and no idea which is theirs.
    pub fn open_with(
        path: impl AsRef<Path>,
        wanted: Option<StorageEngine>,
    ) -> Result<Self, DomainError> {
        let path = path.as_ref();
        let engine = match (engine_of(path)?, wanted) {
            (Some(existing), Some(wanted)) if existing != wanted => {
                tracing::error!(
                    path = %path.display(),
                    existing = existing.name(),
                    requested = wanted.name(),
                    "refusing to open a ceremony store with an engine that did not write it"
                );
                return Err(DomainError::InvariantViolated {
                    reason: "the ceremony store was written by a different storage engine",
                });
            }
            (Some(existing), _) => existing,
            (None, wanted) => wanted.unwrap_or(StorageEngine::Redb),
        };
        Self::on(path, engine)
    }

    pub(super) fn on(path: &Path, engine: StorageEngine) -> Result<Self, DomainError> {
        let engine: Arc<dyn Engine> = match engine {
            StorageEngine::Redb => Arc::new(RedbEngine::open(path)?),
            #[cfg(feature = "sqlite")]
            StorageEngine::Sqlite => Arc::new(crate::engine::sqlite::SqliteEngine::open(path)?),
            #[cfg(not(feature = "sqlite"))]
            StorageEngine::Sqlite => {
                tracing::error!(
                    path = %path.display(),
                    "the ceremony store is a SQLite database and this binary was built without that engine"
                );
                return Err(DomainError::InvariantViolated {
                    reason: "the ceremony store needs the sqlite engine, which this binary was \
                             built without; rebuild with --features sqlite",
                });
            }
        };
        Ok(Self { engine })
    }
}
