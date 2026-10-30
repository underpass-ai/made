//! [`MountProtocolDefinitionsUseCase`] — load and persist protocol definitions.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{ProtocolDefinitionRepositoryPort, ProtocolDefinitionSourcePort};

use super::mount_protocol_definitions_output::MountProtocolDefinitionsOutput;

pub struct MountProtocolDefinitionsUseCase {
    source: Arc<dyn ProtocolDefinitionSourcePort>,
    repository: Arc<dyn ProtocolDefinitionRepositoryPort>,
}

impl std::fmt::Debug for MountProtocolDefinitionsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountProtocolDefinitionsUseCase").finish()
    }
}

impl MountProtocolDefinitionsUseCase {
    #[must_use]
    pub fn new(
        source: Arc<dyn ProtocolDefinitionSourcePort>,
        repository: Arc<dyn ProtocolDefinitionRepositoryPort>,
    ) -> Self {
        Self { source, repository }
    }

    #[tracing::instrument(name = "mount_protocol_definitions", skip_all)]
    pub async fn execute(&self) -> Result<MountProtocolDefinitionsOutput, DomainError> {
        let definitions = self.source.load().await?;
        if definitions.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "protocol_definitions",
            });
        }

        for definition in &definitions {
            self.repository.save(definition).await?;
        }

        Ok(MountProtocolDefinitionsOutput::new(definitions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use made_core::entities::ProtocolDefinition;
    use made_core::value_objects::{ProtocolName, ProtocolState, ProtocolVersion, StateId};
    use tokio::sync::RwLock;

    #[derive(Debug)]
    struct FixedSource {
        definitions: Vec<ProtocolDefinition>,
    }

    #[async_trait]
    impl ProtocolDefinitionSourcePort for FixedSource {
        async fn load(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
            Ok(self.definitions.clone())
        }
    }

    #[derive(Debug, Default)]
    struct RecordingRepository {
        saved: RwLock<Vec<ProtocolDefinition>>,
    }

    #[async_trait]
    impl ProtocolDefinitionRepositoryPort for RecordingRepository {
        async fn save(&self, definition: &ProtocolDefinition) -> Result<(), DomainError> {
            self.saved.write().await.push(definition.clone());
            Ok(())
        }

        async fn get(
            &self,
            _name: &ProtocolName,
            _version: &ProtocolVersion,
        ) -> Result<ProtocolDefinition, DomainError> {
            Err(DomainError::NotFound {
                what: "protocol_definition",
            })
        }

        async fn list(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
            Ok(self.saved.read().await.clone())
        }
    }

    fn definition() -> ProtocolDefinition {
        ProtocolDefinition::new(
            ProtocolName::new("planning_protocol").unwrap(),
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
    async fn mounts_loaded_definitions_into_repository() {
        let repository = Arc::new(RecordingRepository::default());
        let usecase = MountProtocolDefinitionsUseCase::new(
            Arc::new(FixedSource {
                definitions: vec![definition()],
            }),
            repository.clone(),
        );

        let output = usecase.execute().await.unwrap();

        assert_eq!(output.definitions().len(), 1);
        assert_eq!(repository.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn empty_source_is_rejected() {
        let repository = Arc::new(RecordingRepository::default());
        let usecase = MountProtocolDefinitionsUseCase::new(
            Arc::new(FixedSource {
                definitions: Vec::new(),
            }),
            repository,
        );

        let err = usecase.execute().await.unwrap_err();

        assert!(matches!(
            err,
            DomainError::EmptyCollection {
                field: "protocol_definitions"
            }
        ));
    }
}
