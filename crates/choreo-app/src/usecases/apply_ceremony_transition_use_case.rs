//! [`ApplyCeremonyTransitionUseCase`] — apply a guarded ceremony transition.

use std::sync::Arc;

use choreo_core::entities::CeremonyInstance;
use choreo_core::error::DomainError;
use choreo_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyInstanceRepositoryPort, ClockPort,
};

use super::apply_ceremony_transition_input::ApplyCeremonyTransitionInput;

pub struct ApplyCeremonyTransitionUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    instances: Arc<dyn CeremonyInstanceRepositoryPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for ApplyCeremonyTransitionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplyCeremonyTransitionUseCase").finish()
    }
}

impl ApplyCeremonyTransitionUseCase {
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
        name = "apply_ceremony_transition",
        skip_all,
        fields(ceremony_id = %input.instance_id, trigger = %input.trigger)
    )]
    pub async fn execute(
        &self,
        input: ApplyCeremonyTransitionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let definition = self
            .definitions
            .get(&input.definition_name, &input.definition_version)
            .await?;
        let mut instance = self.instances.get(&input.instance_id).await?;
        instance.apply_transition_as(
            &definition,
            &input.role_id,
            &input.trigger,
            self.clock.now(),
        )?;
        self.instances.save(&instance).await?;
        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use choreo_core::error::DomainError;
    use choreo_core::ports::CeremonyInstanceRepositoryPort;
    use choreo_core::value_objects::{StateId, StepOutput, StepResult};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_name, now, role_id, started_instance, step_id, trigger,
        version, DefinitionRepositoryFake, FixedClock, InstanceRepositoryFake,
    };

    #[tokio::test]
    async fn applies_guarded_transition_after_step_completion() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(InstanceRepositoryFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                choreo_core::value_objects::StepLease::new(
                    crate::usecases::ceremony_test_support::lease_owner(),
                    crate::usecases::ceremony_test_support::idempotency_key("lease-1"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definitions,
            instances.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let transitioned = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                role_id(),
                trigger(),
            ))
            .await
            .unwrap();

        assert_eq!(
            transitioned.current_state(),
            &StateId::new("COMPLETED").unwrap()
        );
        assert!(instances
            .saved(&ceremony_id())
            .await
            .is_completed(&definition));
    }

    #[tokio::test]
    async fn unsatisfied_guard_is_rejected() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(InstanceRepositoryFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definitions,
            instances,
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                role_id(),
                trigger(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }
}
