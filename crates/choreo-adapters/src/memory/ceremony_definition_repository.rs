//! In-memory [`CeremonyDefinitionRepositoryPort`] implementation.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use choreo_core::entities::CeremonyDefinition;
use choreo_core::error::DomainError;
use choreo_core::ports::CeremonyDefinitionRepositoryPort;
use choreo_core::value_objects::{CeremonyName, CeremonyVersion};
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone)]
pub struct InMemoryCeremonyDefinitionRepository {
    inner: Arc<RwLock<BTreeMap<(CeremonyName, CeremonyVersion), CeremonyDefinition>>>,
}

impl InMemoryCeremonyDefinitionRepository {
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
impl CeremonyDefinitionRepositoryPort for InMemoryCeremonyDefinitionRepository {
    async fn save(&self, definition: &CeremonyDefinition) -> Result<(), DomainError> {
        self.inner.write().await.insert(
            (definition.name().clone(), definition.version().clone()),
            definition.clone(),
        );
        Ok(())
    }

    async fn get(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError> {
        self.inner
            .read()
            .await
            .get(&(name.clone(), version.clone()))
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_definition",
            })
    }

    async fn list(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::value_objects::{CeremonyState, StateId};

    fn definition() -> CeremonyDefinition {
        definition_named("planning_ceremony")
    }

    fn definition_named(name: &str) -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new(name).unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(StateId::new("STARTED").unwrap())],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn save_then_get_roundtrips() {
        let repository = InMemoryCeremonyDefinitionRepository::new();
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
        let repository = InMemoryCeremonyDefinitionRepository::new();
        repository
            .save(&definition_named("first_ceremony"))
            .await
            .unwrap();
        repository
            .save(&definition_named("second_ceremony"))
            .await
            .unwrap();

        let listed = repository.list().await.unwrap();

        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn save_overwrites_same_name_and_version() {
        let repository = InMemoryCeremonyDefinitionRepository::new();
        let definition = definition();

        repository.save(&definition).await.unwrap();
        repository.save(&definition).await.unwrap();

        assert_eq!(repository.len().await, 1);
        assert!(!repository.is_empty().await);
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let repository = InMemoryCeremonyDefinitionRepository::new();
        let clone = repository.clone();

        repository.save(&definition()).await.unwrap();

        assert_eq!(clone.len().await, 1);
    }

    #[tokio::test]
    async fn missing_definition_returns_not_found() {
        let repository = InMemoryCeremonyDefinitionRepository::new();

        let err = repository
            .get(
                &CeremonyName::new("missing").unwrap(),
                &CeremonyVersion::v1(),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "ceremony_definition"
            }
        ));
    }
}
