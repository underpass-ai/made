//! [`StartCeremonyUseCase`] — create a ceremony instance from a definition.

use std::sync::Arc;

use choreo_core::entities::CeremonyInstance;
use choreo_core::error::DomainError;
use choreo_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyInstanceRepositoryPort, ClockPort,
};

use super::start_ceremony_input::StartCeremonyInput;

pub struct StartCeremonyUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    instances: Arc<dyn CeremonyInstanceRepositoryPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for StartCeremonyUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartCeremonyUseCase").finish()
    }
}

impl StartCeremonyUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        instances: Arc<dyn CeremonyInstanceRepositoryPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            instances,
            clock,
        }
    }

    #[tracing::instrument(
        name = "start_ceremony",
        skip_all,
        fields(ceremony_id = %input.id)
    )]
    pub async fn execute(
        &self,
        input: StartCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        if self.instances.exists(&input.id).await? {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance",
            });
        }

        let definition = self
            .definitions
            .get(&input.definition_name, &input.definition_version)
            .await?;
        let instance =
            CeremonyInstance::start(input.id, &definition, input.context, self.clock.now());
        self.instances.save(&instance).await?;
        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use choreo_core::error::DomainError;
    use choreo_core::ports::CeremonyInstanceRepositoryPort;
    use choreo_core::value_objects::{CeremonyContext, StateId};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_name, now, started_instance, version,
        DefinitionRepositoryFake, FixedClock, InstanceRepositoryFake,
    };

    #[tokio::test]
    async fn starts_and_persists_instance_at_initial_state() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition));
        let instances = Arc::new(InstanceRepositoryFake::default());
        let usecase = StartCeremonyUseCase::new(
            definitions,
            instances.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let instance = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
            ))
            .await
            .unwrap();

        assert_eq!(
            instance.current_state(),
            &StateId::new("COLLECTING_VOICES").unwrap()
        );
        let saved = instances.saved(&ceremony_id()).await;
        assert_eq!(saved.id(), &ceremony_id());
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected_before_overwrite() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(InstanceRepositoryFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase =
            StartCeremonyUseCase::new(definitions, instances, Arc::new(FixedClock::new(now())));

        let err = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
    }
}
