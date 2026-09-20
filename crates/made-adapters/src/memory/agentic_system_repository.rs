use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};
use tokio::sync::RwLock;

/// Process-local designs, every revision kept in order.
///
/// A vector per design rather than a head plus history: the revision
/// log *is* the storage, and a store that kept only the head would
/// pass the conflict test and fail the one that reads back what a
/// design said before somebody changed it.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAgenticSystemRepository {
    inner: Arc<RwLock<BTreeMap<AgenticSystemId, Vec<AgenticSystem>>>>,
}

impl InMemoryAgenticSystemRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgenticSystemRepositoryPort for InMemoryAgenticSystemRepository {
    async fn save(
        &self,
        system: AgenticSystem,
        expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        let mut designs = self.inner.write().await;
        let history = designs.entry(system.id().clone()).or_default();
        let head = history.last().map(AgenticSystem::revision);
        if head != expected {
            return Ok(AgenticSystemSaveOutcome::conflict(
                head.unwrap_or(AgenticSystemRevision::INITIAL),
            ));
        }
        let revision = head.map_or(AgenticSystemRevision::INITIAL, AgenticSystemRevision::next);
        history.push(system.at_revision(revision));
        Ok(AgenticSystemSaveOutcome::saved(revision))
    }

    async fn get(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        let designs = self.inner.read().await;
        let Some(history) = designs.get(id) else {
            return Ok(None);
        };
        Ok(match revision {
            Some(revision) => history
                .iter()
                .find(|system| system.revision() == revision)
                .cloned(),
            None => history.last().cloned(),
        })
    }

    async fn list(&self, query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        let designs = self.inner.read().await;
        let mut admitted: Vec<AgenticSystem> = designs
            .values()
            .filter_map(|history| history.last())
            .filter(|system| query.admits(system.id(), system.lifecycle()))
            .cloned()
            .collect();
        let next_cursor = (admitted.len() > query.limit().as_usize())
            .then(|| admitted[query.limit().as_usize() - 1].id().clone());
        admitted.truncate(query.limit().as_usize());
        Ok(AgenticSystemPage::new(admitted, next_cursor))
    }
}
