//! Runs of a design in the canonical embedded SQLite store.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemExecutionUpdate,
};
use made_core::value_objects::{AgenticSystemExecutionId, AgenticSystemId};
use time::OffsetDateTime;

use crate::engine::{Engine, Key, Table};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::SqliteCeremonyStore;

/// What each run of each design has got to, durable across restarts.
#[derive(Debug, Clone)]
pub struct SqliteAgenticSystemExecutions {
    engine: Arc<dyn Engine>,
}

impl SqliteAgenticSystemExecutions {
    /// Open the runs through the canonical store's lifecycle checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.agentic_system_executions())
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
impl AgenticSystemExecutionStorePort for SqliteAgenticSystemExecutions {
    /// Looked up and written in one transaction, so a host that asks
    /// twice because it did not hear the first answer gets the run it
    /// already opened rather than a second one beside it.
    async fn create(
        &self,
        execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        self.blocking("create agentic system execution", move |engine| {
            let key = execution.id().to_string();
            let mut tx = engine.begin_write()?;
            if let Some(existing) = tx.get(Table::AgenticSystemExecutions, Key::Str(&key))? {
                return Ok(AgenticSystemExecutionCreation::AlreadyExists(decode(
                    &existing,
                    "decode agentic system execution",
                )?));
            }
            tx.insert(
                Table::AgenticSystemExecutions,
                Key::Str(&key),
                &encode(&execution, "encode agentic system execution")?,
            )?;
            tx.commit()?;
            Ok(AgenticSystemExecutionCreation::Created(execution))
        })
        .await
    }

    async fn get(
        &self,
        id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        let key = id.to_string();
        self.blocking("read agentic system execution", move |engine| {
            let tx = engine.begin_read()?;
            tx.get(Table::AgenticSystemExecutions, Key::Str(&key))?
                .map(|value| decode(&value, "decode agentic system execution"))
                .transpose()
        })
        .await
    }

    async fn update(
        &self,
        execution: AgenticSystemExecution,
        expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        self.blocking("update agentic system execution", move |engine| {
            let key = execution.id().to_string();
            let mut tx = engine.begin_write()?;
            let stored: AgenticSystemExecution = tx
                .get(Table::AgenticSystemExecutions, Key::Str(&key))?
                .map(|value| decode(&value, "decode agentic system execution"))
                .transpose()?
                .ok_or(DomainError::NotFound {
                    what: "agentic_system_execution",
                })?;
            if stored.updated_at() != expected_updated_at {
                return Ok(AgenticSystemExecutionUpdate::Conflict(stored));
            }
            tx.insert(
                Table::AgenticSystemExecutions,
                Key::Str(&key),
                &encode(&execution, "encode agentic system execution")?,
            )?;
            tx.commit()?;
            Ok(AgenticSystemExecutionUpdate::Updated(execution))
        })
        .await
    }

    /// A full scan, filtered by the design each run pinned.
    ///
    /// No second index: a run's key is its own identity, an index from
    /// design to run would be a second thing to keep true, and the
    /// number of runs one store holds is bounded by how much work an
    /// operator has actually started.
    async fn list_by_system(
        &self,
        id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        let id = id.clone();
        self.blocking("list agentic system executions", move |engine| {
            let tx = engine.begin_read()?;
            let mut found: Vec<AgenticSystemExecution> = tx
                .scan_str(Table::AgenticSystemExecutions)?
                .into_iter()
                .map(|(_, value)| decode(&value, "decode agentic system execution"))
                .collect::<Result<Vec<_>, _>>()?;
            found.retain(|execution| execution.system().id() == &id);
            found.sort_by_key(AgenticSystemExecution::created_at);
            Ok(found)
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// Runs sharing this store's open engine and pool.
    #[must_use]
    pub fn agentic_system_executions(&self) -> SqliteAgenticSystemExecutions {
        SqliteAgenticSystemExecutions::from_engine(Arc::clone(&self.engine))
    }
}
