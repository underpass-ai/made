use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AgenticSystemPublicationOutcome, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::AgenticSystemPublicationPort;
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};
use tokio::sync::RwLock;

/// Process-local sealed designs, one slot per revision.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAgenticSystemPublications {
    inner: Arc<RwLock<BTreeMap<(AgenticSystemId, AgenticSystemRevision), PublishedAgenticSystem>>>,
}

impl InMemoryAgenticSystemPublications {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgenticSystemPublicationPort for InMemoryAgenticSystemPublications {
    async fn publish(
        &self,
        published: PublishedAgenticSystem,
    ) -> Result<AgenticSystemPublicationOutcome, DomainError> {
        let mut sealed = self.inner.write().await;
        let slot = (published.id().clone(), published.revision());
        Ok(match sealed.get(&slot) {
            Some(occupant) if occupant.digest() == published.digest() => {
                AgenticSystemPublicationOutcome::AlreadyPublished(occupant.clone())
            }
            Some(occupant) => AgenticSystemPublicationOutcome::RevisionOccupied {
                published: occupant.digest(),
                offered: published.digest(),
            },
            None => {
                sealed.insert(slot, published.clone());
                AgenticSystemPublicationOutcome::Published(published)
            }
        })
    }

    async fn published(
        &self,
        id: &AgenticSystemId,
        revision: AgenticSystemRevision,
    ) -> Result<Option<PublishedAgenticSystem>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(&(id.clone(), revision))
            .cloned())
    }

    async fn catalogue(&self) -> Result<Vec<PublishedAgenticSystem>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}
