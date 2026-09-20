//! Sealed designs in the canonical embedded SQLite store.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AgenticSystemPublicationOutcome, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::AgenticSystemPublicationPort;
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};

use crate::engine::{Engine, Key, Table};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::keys::agentic_system_revision;
use super::SqliteCeremonyStore;

/// Revisions that can no longer change, durable across restarts.
#[derive(Debug, Clone)]
pub struct SqliteAgenticSystemPublications {
    engine: Arc<dyn Engine>,
}

impl SqliteAgenticSystemPublications {
    /// Open the publications through the canonical store's lifecycle
    /// checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.agentic_system_publications())
    }

    pub(super) fn from_engine(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }

    async fn blocking<T, F>(&self, op: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, op))?
    }
}

#[async_trait]
impl AgenticSystemPublicationPort for SqliteAgenticSystemPublications {
    /// The occupant is read and the slot written inside one write
    /// transaction, so two callers cannot seal different content under
    /// one revision.
    async fn publish(
        &self,
        published: PublishedAgenticSystem,
    ) -> Result<AgenticSystemPublicationOutcome, DomainError> {
        self.blocking("publish agentic system", move |engine| {
            let key = agentic_system_revision(published.id(), published.revision().get());
            let mut tx = engine.begin_write()?;
            let occupant: Option<PublishedAgenticSystem> = tx
                .get(Table::AgenticSystemPublications, Key::Str(&key))?
                .map(|value| decode(&value, "decode published agentic system"))
                .transpose()?;
            let outcome = match occupant {
                Some(occupant) if occupant.digest() == published.digest() => {
                    AgenticSystemPublicationOutcome::AlreadyPublished(occupant)
                }
                Some(occupant) => AgenticSystemPublicationOutcome::RevisionOccupied {
                    published: occupant.digest(),
                    offered: published.digest(),
                },
                None => {
                    tx.insert(
                        Table::AgenticSystemPublications,
                        Key::Str(&key),
                        &encode(&published, "encode published agentic system")?,
                    )?;
                    AgenticSystemPublicationOutcome::Published(published)
                }
            };
            if outcome.is_new() {
                tx.commit()?;
            }
            Ok(outcome)
        })
        .await
    }

    async fn published(
        &self,
        id: &AgenticSystemId,
        revision: AgenticSystemRevision,
    ) -> Result<Option<PublishedAgenticSystem>, DomainError> {
        let key = agentic_system_revision(id, revision.get());
        self.blocking("read published agentic system", move |engine| {
            let tx = engine.begin_read()?;
            tx.get(Table::AgenticSystemPublications, Key::Str(&key))?
                .map(|value| decode(&value, "decode published agentic system"))
                .transpose()
        })
        .await
    }

    async fn catalogue(&self) -> Result<Vec<PublishedAgenticSystem>, DomainError> {
        self.blocking("catalogue agentic systems", move |engine| {
            let tx = engine.begin_read()?;
            tx.scan_str(Table::AgenticSystemPublications)?
                .into_iter()
                .map(|(_, value)| decode(&value, "decode published agentic system"))
                .collect()
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// Sealed designs sharing this store's open engine and pool.
    #[must_use]
    pub fn agentic_system_publications(&self) -> SqliteAgenticSystemPublications {
        SqliteAgenticSystemPublications::from_engine(Arc::clone(&self.engine))
    }
}
