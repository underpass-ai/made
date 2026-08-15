//! [`MountCeremonyDefinitionsUseCase`] — load and persist ceremony definitions.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionRepositoryPort, CeremonyDefinitionSourcePort};

use super::mount_ceremony_definitions_output::MountCeremonyDefinitionsOutput;

pub struct MountCeremonyDefinitionsUseCase {
    source: Arc<dyn CeremonyDefinitionSourcePort>,
    repository: Arc<dyn CeremonyDefinitionRepositoryPort>,
}

impl std::fmt::Debug for MountCeremonyDefinitionsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountCeremonyDefinitionsUseCase").finish()
    }
}

impl MountCeremonyDefinitionsUseCase {
    #[must_use]
    pub fn new(
        source: Arc<dyn CeremonyDefinitionSourcePort>,
        repository: Arc<dyn CeremonyDefinitionRepositoryPort>,
    ) -> Self {
        Self { source, repository }
    }

    #[tracing::instrument(name = "mount_ceremony_definitions", skip_all)]
    pub async fn execute(&self) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        let definitions = self.source.load().await?;
        if definitions.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "ceremony_definitions",
            });
        }

        for definition in &definitions {
            self.repository.save(definition).await?;
        }

        Ok(MountCeremonyDefinitionsOutput::new(definitions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use made_core::entities::CeremonyDefinition;
    use made_core::value_objects::{CeremonyName, CeremonyState, CeremonyVersion, StateId};
    use tokio::sync::RwLock;

    #[derive(Debug)]
    struct FixedSource {
        definitions: Vec<CeremonyDefinition>,
    }

    #[async_trait]
    impl CeremonyDefinitionSourcePort for FixedSource {
        async fn load(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
            Ok(self.definitions.clone())
        }
    }

    #[derive(Debug, Default)]
    struct RecordingRepository {
        saved: RwLock<Vec<CeremonyDefinition>>,
    }

    #[async_trait]
    impl CeremonyDefinitionRepositoryPort for RecordingRepository {
        async fn save(&self, definition: &CeremonyDefinition) -> Result<(), DomainError> {
            self.saved.write().await.push(definition.clone());
            Ok(())
        }

        async fn get(
            &self,
            _name: &CeremonyName,
            _version: &CeremonyVersion,
        ) -> Result<CeremonyDefinition, DomainError> {
            Err(DomainError::NotFound {
                what: "ceremony_definition",
            })
        }

        async fn list(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
            Ok(self.saved.read().await.clone())
        }
    }

    fn definition() -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new("planning_ceremony").unwrap(),
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
    async fn mounts_loaded_definitions_into_repository() {
        let repository = Arc::new(RecordingRepository::default());
        let usecase = MountCeremonyDefinitionsUseCase::new(
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
        let usecase = MountCeremonyDefinitionsUseCase::new(
            Arc::new(FixedSource {
                definitions: Vec::new(),
            }),
            repository,
        );

        let err = usecase.execute().await.unwrap_err();

        assert!(matches!(
            err,
            DomainError::EmptyCollection {
                field: "ceremony_definitions"
            }
        ));
    }
}
