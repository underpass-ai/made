//! [`CompleteCeremonyStepUseCase`] — apply a step result to a ceremony.

use std::sync::Arc;

use choreo_core::entities::CeremonyInstance;
use choreo_core::error::DomainError;
use choreo_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyInstanceRepositoryPort, ClockPort,
};

use super::complete_ceremony_step_input::CompleteCeremonyStepInput;

pub struct CompleteCeremonyStepUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    instances: Arc<dyn CeremonyInstanceRepositoryPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CompleteCeremonyStepUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompleteCeremonyStepUseCase").finish()
    }
}

impl CompleteCeremonyStepUseCase {
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
        name = "complete_ceremony_step",
        skip_all,
        fields(ceremony_id = %input.instance_id, step_id = %input.step_id)
    )]
    pub async fn execute(
        &self,
        input: CompleteCeremonyStepInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let definition = self
            .definitions
            .get(&input.definition_name, &input.definition_version)
            .await?;
        let mut instance = self.instances.get(&input.instance_id).await?;
        instance.apply_step_result(&definition, &input.step_id, input.result, self.clock.now())?;
        self.instances.save(&instance).await?;
        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use choreo_core::ports::CeremonyInstanceRepositoryPort;
    use choreo_core::value_objects::{StepOutput, StepResult, StepStatus};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_name, idempotency_key, lease_owner, now, role_id,
        started_instance, step_id, version, DefinitionRepositoryFake, FixedClock,
        InstanceRepositoryFake,
    };

    #[tokio::test]
    async fn applies_step_result_and_clears_lease() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(InstanceRepositoryFake::default());
        let mut instance = started_instance(&definition);
        let lease = choreo_core::value_objects::StepLease::new(
            lease_owner(),
            idempotency_key("lease-1"),
            now(),
            now() + time::Duration::seconds(60),
        )
        .unwrap();
        instance
            .start_step_as(&definition, &role_id(), &step_id(), lease, now())
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = CompleteCeremonyStepUseCase::new(
            definitions,
            instances.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let completed = usecase
            .execute(CompleteCeremonyStepInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
            ))
            .await
            .unwrap();

        let record = completed.step_record(&step_id()).unwrap();
        assert_eq!(record.status(), StepStatus::Completed);
        assert!(record.lease().is_none());
        assert_eq!(
            instances
                .saved(&ceremony_id())
                .await
                .step_record(&step_id())
                .unwrap()
                .status(),
            StepStatus::Completed
        );
    }
}
