//! In-memory [`ProtocolDefinitionPublicationPort`] implementation.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{PublicationOutcome, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionPublicationPort;
use made_core::value_objects::{ProtocolName, ProtocolVersion};
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone)]
pub struct InMemoryProtocolDefinitionPublications {
    inner: Arc<RwLock<BTreeMap<(ProtocolName, ProtocolVersion), PublishedProtocolDefinition>>>,
}

impl InMemoryProtocolDefinitionPublications {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ProtocolDefinitionPublicationPort for InMemoryProtocolDefinitionPublications {
    /// The occupant is read and the slot written under one write lock:
    /// releasing it in between would let two callers publish different
    /// content under one version.
    async fn publish(
        &self,
        definition: PublishedProtocolDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        let key = (definition.name().clone(), definition.version().clone());
        let mut published = self.inner.write().await;

        Ok(match published.get(&key) {
            Some(occupant) if occupant.holds_same_content_as(&definition)? => {
                PublicationOutcome::AlreadyPublished(occupant.clone())
            }
            Some(occupant) => PublicationOutcome::VersionOccupied {
                published: occupant.digest(),
                offered: definition.digest(),
            },
            None => {
                published.insert(key, definition.clone());
                PublicationOutcome::Published(definition)
            }
        })
    }

    async fn published(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<Option<PublishedProtocolDefinition>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(&(name.clone(), version.clone()))
            .cloned())
    }

    async fn catalogue(&self) -> Result<Vec<PublishedProtocolDefinition>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}
