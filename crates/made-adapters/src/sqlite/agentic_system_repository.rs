//! Designs in the canonical embedded SQLite store.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};

use crate::engine::{Engine, Key, ReadTx, Table};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::keys::{agentic_system_prefix, agentic_system_revision};
use super::SqliteCeremonyStore;

/// The revision log of agentic system designs, durable across
/// restarts.
#[derive(Debug, Clone)]
pub struct SqliteAgenticSystemRepository {
    engine: Arc<dyn Engine>,
}

impl SqliteAgenticSystemRepository {
    /// Open the repository through the canonical store's lifecycle
    /// checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.agentic_system_repository())
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
impl AgenticSystemRepositoryPort for SqliteAgenticSystemRepository {
    /// The head is read and the new revision written inside one write
    /// transaction. Read first and written after, two editors would
    /// both find the same head and both believe they had it.
    async fn save(
        &self,
        system: AgenticSystem,
        expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        self.blocking("save agentic system", move |engine| {
            let mut tx = engine.begin_write()?;
            let head = head_revision(tx.as_ref(), system.id())?;
            if head != expected {
                return Ok(match head {
                    Some(current) => AgenticSystemSaveOutcome::conflict(current),
                    // Expected a revision that is not there: the
                    // design the editor read has never been stored.
                    None => AgenticSystemSaveOutcome::conflict(AgenticSystemRevision::INITIAL),
                });
            }
            let revision = match head {
                Some(current) => current.next(),
                None => AgenticSystemRevision::INITIAL,
            };
            let stored = system.at_revision(revision);
            tx.insert(
                Table::AgenticSystems,
                Key::Str(&agentic_system_revision(stored.id(), revision.get())),
                &encode(&stored, "encode agentic system")?,
            )?;
            tx.commit()?;
            Ok(AgenticSystemSaveOutcome::saved(revision))
        })
        .await
    }

    async fn get(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        let id = id.clone();
        self.blocking("read agentic system", move |engine| {
            let tx = engine.begin_read()?;
            match revision {
                Some(revision) => tx
                    .get(
                        Table::AgenticSystems,
                        Key::Str(&agentic_system_revision(&id, revision.get())),
                    )?
                    .map(|value| decode(&value, "decode agentic system"))
                    .transpose(),
                None => head(tx.as_ref(), &id),
            }
        })
        .await
    }

    /// Heads only, in identifier order.
    ///
    /// The store holds every revision, so a page of designs is a page
    /// of the last revision of each: listing what somebody edited
    /// three times should show them one system, not three.
    async fn list(&self, query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        let query = query.clone();
        self.blocking("list agentic systems", move |engine| {
            let tx = engine.begin_read()?;
            let mut admitted = Vec::new();
            let mut next_cursor = None;
            let mut after = query.after().map(agentic_system_prefix);
            loop {
                let rows = tx.scan_str_page(
                    Table::AgenticSystems,
                    after.as_deref(),
                    None,
                    // Revisions share a design, so a page of rows is
                    // not a page of designs; read in blocks and stop
                    // once enough designs have been collected.
                    query.limit().as_usize() * 4,
                )?;
                if rows.is_empty() {
                    break;
                }
                after = rows.last().map(|(key, _)| key.clone());
                for (_, value) in rows {
                    let system: AgenticSystem = decode(&value, "decode agentic system")?;
                    let Some(head) = head(tx.as_ref(), system.id())? else {
                        continue;
                    };
                    if head.revision() != system.revision()
                        || !query.admits(system.id(), system.lifecycle())
                    {
                        continue;
                    }
                    if admitted.len() == query.limit().as_usize() {
                        next_cursor = Some(
                            admitted
                                .last()
                                .map(|last: &AgenticSystem| last.id().clone())
                                .expect("a full page has a last entry"),
                        );
                        break;
                    }
                    admitted.push(system);
                }
                if next_cursor.is_some() {
                    break;
                }
            }
            Ok(AgenticSystemPage::new(admitted, next_cursor))
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// Designs sharing this store's open engine and pool.
    #[must_use]
    pub fn agentic_system_repository(&self) -> SqliteAgenticSystemRepository {
        SqliteAgenticSystemRepository::from_engine(Arc::clone(&self.engine))
    }
}

/// The last revision of one design, or nothing when it has none.
fn head(tx: &dyn ReadTx, id: &AgenticSystemId) -> Result<Option<AgenticSystem>, DomainError> {
    let prefix = agentic_system_prefix(id);
    let mut latest: Option<AgenticSystem> = None;
    let mut after: Option<String> = None;
    loop {
        let rows = tx.scan_str_page(Table::AgenticSystems, after.as_deref(), Some(&prefix), 64)?;
        if rows.is_empty() {
            return Ok(latest);
        }
        after = rows.last().map(|(key, _)| key.clone());
        for (_, value) in rows {
            latest = Some(decode(&value, "decode agentic system")?);
        }
    }
}

fn head_revision(
    tx: &dyn ReadTx,
    id: &AgenticSystemId,
) -> Result<Option<AgenticSystemRevision>, DomainError> {
    Ok(head(tx, id)?.map(|system| system.revision()))
}
