//! In-memory [`ProtocolDefinitionRepositoryPort`] implementation.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionRepositoryPort;
use made_core::value_objects::{ProtocolName, ProtocolVersion};
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone)]
pub struct InMemoryProtocolDefinitionRepository {
    inner: Arc<RwLock<BTreeMap<(ProtocolName, ProtocolVersion), ProtocolDefinition>>>,
}

impl InMemoryProtocolDefinitionRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

#[async_trait]
impl ProtocolDefinitionRepositoryPort for InMemoryProtocolDefinitionRepository {
    async fn save(&self, definition: &ProtocolDefinition) -> Result<(), DomainError> {
        self.inner.write().await.insert(
            (definition.name().clone(), definition.version().clone()),
            definition.clone(),
        );
        Ok(())
    }

    async fn get(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<ProtocolDefinition, DomainError> {
        self.inner
            .read()
            .await
            .get(&(name.clone(), version.clone()))
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "protocol_definition",
            })
    }

    async fn list(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{ProtocolState, StateId};

    fn definition() -> ProtocolDefinition {
        definition_named("planning_protocol")
    }

    fn definition_named(name: &str) -> ProtocolDefinition {
        ProtocolDefinition::new(
            ProtocolName::new(name).unwrap(),
            ProtocolVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![ProtocolState::initial(StateId::new("STARTED").unwrap())],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn save_then_get_roundtrips() {
        let repository = InMemoryProtocolDefinitionRepository::new();
        let definition = definition();

        repository.save(&definition).await.unwrap();
        let got = repository
            .get(definition.name(), definition.version())
            .await
            .unwrap();

        assert_eq!(got.name(), definition.name());
    }

    #[tokio::test]
    async fn list_returns_saved_definitions() {
        let repository = InMemoryProtocolDefinitionRepository::new();
        repository
            .save(&definition_named("first_protocol"))
            .await
            .unwrap();
        repository
            .save(&definition_named("second_protocol"))
            .await
            .unwrap();

        let listed = repository.list().await.unwrap();

        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn save_overwrites_same_name_and_version() {
        let repository = InMemoryProtocolDefinitionRepository::new();
        let definition = definition();

        repository.save(&definition).await.unwrap();
        repository.save(&definition).await.unwrap();

        assert_eq!(repository.len().await, 1);
        assert!(!repository.is_empty().await);
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let repository = InMemoryProtocolDefinitionRepository::new();
        let clone = repository.clone();

        repository.save(&definition()).await.unwrap();

        assert_eq!(clone.len().await, 1);
    }

    #[tokio::test]
    async fn missing_definition_returns_not_found() {
        let repository = InMemoryProtocolDefinitionRepository::new();

        let err = repository
            .get(
                &ProtocolName::new("missing").unwrap(),
                &ProtocolVersion::v1(),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_definition"
            }
        ));
    }
}
