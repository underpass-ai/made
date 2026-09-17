use crate::entities::ceremony_commands::{ApplyStepResult, StartStep};
use crate::entities::ceremony_events::{StepCompleted, StepFailed, StepStarted};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{RoleAction, StepAttempt, StepExecutionRecord, StepStatus};

impl CeremonyInstance {
    /// A step may be taken when its state is current, its lease is
    /// free or expired, its retry policy has attempts left and its
    /// idempotency key is new. The event names the seat that took it:
    /// the one the command declares, once the definition allows it
    /// the step, or the seat the definition assigns to the step when
    /// the engine took it.
    pub(super) fn decide_start_step(
        &self,
        command: &StartStep,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        if let Some(role_id) = command.role_id.as_ref() {
            self.require_role(
                definition,
                role_id,
                &RoleAction::step(command.step_id.clone()),
            )?;
        }
        self.require_definition(definition)?;
        if self.is_terminal(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "terminal ceremony instances cannot start steps",
            });
        }

        let step = definition
            .step(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step",
            })?;
        if step.state_id() != &self.current_state {
            return Err(DomainError::InvalidTransition {
                from: "ceremony_instance.current_state",
                to: "ceremony_step.state",
            });
        }

        let claimable =
            self.claimable_step_ids_at(definition, command.now, command.max_parallel_ceiling)?;
        if !claimable.contains(&&command.step_id) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony step is not claimable at the observed time and capacity",
            });
        }

        let record = self
            .step_records
            .get(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        let attempt = next_attempt_for_start(record)?;
        if self
            .idempotency_keys
            .contains(command.lease.idempotency_key())
        {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance.idempotency_key",
            });
        }
        let started_by = match command.role_id.clone() {
            Some(role_id) => role_id,
            None => definition.role_id_for_step(&command.step_id)?,
        };

        Ok(vec![CeremonyEvent::StepStarted(StepStarted {
            step_id: command.step_id.clone(),
            iteration: record.iteration(),
            attempt,
            lease: command.lease.clone(),
            started_by,
            started_at: command.now,
        })])
    }

    /// A result is filed against the in-progress record of a step in
    /// the current state. Whether a success reopens the step at the
    /// next iteration is the repeat policy's call, decided here and
    /// carried in the event so the fold needs no definition.
    pub(super) fn decide_apply_step_result(
        &self,
        command: &ApplyStepResult,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let step = definition
            .step(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step",
            })?;
        if step.state_id() != &self.current_state {
            return Err(DomainError::InvalidTransition {
                from: "ceremony_instance.current_state",
                to: "ceremony_step.state",
            });
        }

        let record = self
            .step_records
            .get(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if record.status() != StepStatus::InProgress {
            return Err(DomainError::InvariantViolated {
                reason: "step result requires an in-progress step",
            });
        }

        let iteration = record.iteration();
        let attempt = record.attempt();
        let result = command.result.clone();
        if !result.is_success() {
            let finished_by = definition.role_id_for_step(&command.step_id)?;
            return Ok(vec![CeremonyEvent::StepFailed(StepFailed {
                step_id: command.step_id.clone(),
                iteration,
                attempt,
                result,
                finished_by,
                finished_at: command.now,
            })]);
        }

        let next_iteration = step
            .repeat_policy()
            .filter(|policy| !policy.is_satisfied(result.output()))
            .filter(|policy| policy.permits_another_iteration(iteration))
            .map(|_| iteration.next())
            .transpose()?;
        let finished_by = definition.role_id_for_step(&command.step_id)?;
        Ok(vec![CeremonyEvent::StepCompleted(StepCompleted {
            step_id: command.step_id.clone(),
            iteration,
            attempt,
            result,
            next_iteration,
            finished_by,
            finished_at: command.now,
        })])
    }
}

fn next_attempt_for_start(record: &StepExecutionRecord) -> Result<StepAttempt, DomainError> {
    if matches!(record.status(), StepStatus::Failed | StepStatus::InProgress) {
        record.attempt().next()
    } else {
        Ok(record.attempt())
    }
}
