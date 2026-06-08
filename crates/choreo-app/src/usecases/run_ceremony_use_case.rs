//! [`RunCeremonyUseCase`] — execute a declarative ceremony to terminal state.

use std::sync::Arc;

use choreo_core::entities::{CeremonyDefinition, CeremonyInstance};
use choreo_core::error::DomainError;
use choreo_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyInstanceRepositoryPort, CeremonyStepHandlerPort,
    CeremonyStepHandlerRequest, ClockPort,
};
use choreo_core::value_objects::{
    DurationMs, IdempotencyKey, LeaseOwnerId, RoleId, StepAttempt, StepErrorMessage, StepId,
    StepLease, StepResult,
};

use super::ceremony_step_trace::CeremonyStepTrace;
use super::run_ceremony_input::RunCeremonyInput;
use super::run_ceremony_output::RunCeremonyOutput;

pub struct RunCeremonyUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    instances: Arc<dyn CeremonyInstanceRepositoryPort>,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for RunCeremonyUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCeremonyUseCase").finish()
    }
}

impl RunCeremonyUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        instances: Arc<dyn CeremonyInstanceRepositoryPort>,
        handler: Arc<dyn CeremonyStepHandlerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            instances,
            handler,
            clock,
        }
    }

    #[tracing::instrument(
        name = "run_ceremony",
        skip_all,
        fields(ceremony_id = %input.id())
    )]
    pub async fn execute(&self, input: RunCeremonyInput) -> Result<RunCeremonyOutput, DomainError> {
        let (id, definition, context, lease_owner_id, lease_ttl) = input.into_parts();
        if self.instances.exists(&id).await? {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance",
            });
        }
        self.definitions.save(&definition).await?;

        let mut instance =
            CeremonyInstance::start(id.clone(), &definition, context, self.clock.now());
        self.instances.save(&instance).await?;

        let max_iterations = definition
            .states()
            .len()
            .saturating_add(definition.transitions().len())
            .saturating_add(1);
        let mut step_traces = Vec::new();
        for _ in 0..max_iterations {
            if instance.is_completed(&definition) {
                return Ok(RunCeremonyOutput::new(definition, instance, step_traces));
            }

            let state_id = instance.current_state().clone();
            let step_ids = definition
                .steps_for_state(&state_id)
                .map(|step| step.id().clone())
                .collect::<Vec<_>>();
            for step_id in step_ids {
                if instance
                    .step_record(&step_id)
                    .is_some_and(|record| record.status().is_success())
                {
                    continue;
                }
                let role_id = definition.role_id_for_step(&step_id)?;
                let (attempt, step_result) = self
                    .run_step(
                        &definition,
                        &mut instance,
                        &role_id,
                        &step_id,
                        &lease_owner_id,
                        lease_ttl,
                        step_traces.len(),
                    )
                    .await?;
                step_traces.push(CeremonyStepTrace::new(
                    state_id.clone(),
                    step_id,
                    role_id,
                    attempt,
                    step_result.status(),
                ));
                if !step_result.is_success() {
                    return Err(DomainError::InvariantViolated {
                        reason: "ceremony step did not complete successfully",
                    });
                }
            }

            if instance.is_completed(&definition) {
                return Ok(RunCeremonyOutput::new(definition, instance, step_traces));
            }
            let transition = definition
                .next_satisfied_transition(&state_id, instance.step_records(), instance.context())
                .ok_or(DomainError::InvariantViolated {
                    reason: "no satisfied ceremony transition is available",
                })?;
            let role_id = definition.role_id_for_transition(transition.trigger())?;
            instance.apply_transition_as(
                &definition,
                &role_id,
                transition.trigger(),
                self.clock.now(),
            )?;
            self.instances.save(&instance).await?;
        }

        Err(DomainError::InvariantViolated {
            reason: "ceremony execution exceeded transition safety limit",
        })
    }

    async fn run_step(
        &self,
        definition: &CeremonyDefinition,
        instance: &mut CeremonyInstance,
        role_id: &RoleId,
        step_id: &StepId,
        lease_owner_id: &LeaseOwnerId,
        lease_ttl: DurationMs,
        trace_index: usize,
    ) -> Result<(StepAttempt, StepResult), DomainError> {
        let step = definition
            .step(step_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let now = self.clock.now();
        let lease = StepLease::acquire(
            lease_owner_id.clone(),
            IdempotencyKey::new(format!(
                "{}:{}:{}",
                instance.id().as_str(),
                step_id.as_str(),
                trace_index + 1
            ))?,
            now,
            lease_ttl,
        )?;
        let attempt = instance.start_step_as(definition, role_id, step_id, lease, now)?;
        self.instances.save(instance).await?;

        let request = CeremonyStepHandlerRequest::new(
            instance.id().clone(),
            instance.definition_name().clone(),
            instance.definition_version().clone(),
            instance.current_state().clone(),
            step.id().clone(),
            step.handler_kind().clone(),
            step.handler_config().clone(),
            instance.context().clone(),
            attempt,
        );
        let step_result = self.execute_handler(request).await?;

        let mut refreshed = self.instances.get(instance.id()).await?;
        refreshed.apply_step_result(definition, step_id, step_result.clone(), self.clock.now())?;
        self.instances.save(&refreshed).await?;
        *instance = refreshed;

        Ok((attempt, step_result))
    }

    async fn execute_handler(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        match self.handler.execute(request).await {
            Ok(result) => Ok(result),
            Err(error) => {
                let message = StepErrorMessage::new(error.to_string())?;
                StepResult::failed(message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use choreo_core::error::DomainError;
    use choreo_core::ports::{CeremonyDefinitionRepositoryPort, CeremonyInstanceRepositoryPort};
    use choreo_core::value_objects::{CeremonyContext, StepOutput, StepResult, StepStatus};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, lease_owner, lease_ttl, now, started_instance, step_id,
        DefinitionRepositoryFake, FixedClock, InstanceRepositoryFake, StepHandlerFake,
    };

    #[tokio::test]
    async fn executes_linear_ceremony_to_terminal_state() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(InstanceRepositoryFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            instances.clone(),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert!(output.instance().is_completed(&definition));
        assert_eq!(output.step_traces().len(), 1);
        assert_eq!(output.step_traces()[0].step_id(), &step_id());
        assert_eq!(output.step_traces()[0].status(), StepStatus::Completed);
        assert_eq!(handler.requests().await.len(), 1);
        assert!(instances
            .saved(&ceremony_id())
            .await
            .is_completed(&definition));
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(InstanceRepositoryFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            instances,
            handler,
            Arc::new(FixedClock::new(now())),
        );
        let input = || {
            RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
            )
        };

        usecase.execute(input()).await.unwrap();
        let err = usecase.execute(input()).await.unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected_before_saving_definition() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(InstanceRepositoryFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions.clone(),
            instances,
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
        assert!(definitions.list().await.unwrap().is_empty());
        assert!(handler.requests().await.is_empty());
    }
}
