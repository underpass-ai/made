use time::OffsetDateTime;

use crate::entities::{CeremonyDefinition, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{MaxParallel, StateExecution, StepAttempt, StepId, StepStatus};

impl CeremonyInstance {
    /// Steps a caller may claim from the current state at one observed instant.
    ///
    /// Concurrent results contain every eligible alternative while capacity
    /// remains. They are not truncated to the number of free slots: whichever
    /// caller wins is persisted first and every optimistic retry recomputes the
    /// set against that new state.
    pub fn claimable_step_ids_at<'a>(
        &self,
        definition: &'a CeremonyDefinition,
        now: OffsetDateTime,
        host_ceiling: MaxParallel,
    ) -> Result<Vec<&'a StepId>, DomainError> {
        self.require_definition(definition)?;
        let state = definition
            .state(&self.current_state)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.current_state",
            })?;
        let steps = definition
            .steps_for_state(&self.current_state)
            .collect::<Vec<_>>();
        if state.execution() == StateExecution::Sequential {
            let Some(step) = steps.into_iter().find(|step| {
                self.step_records
                    .get(step.id())
                    .is_some_and(|record| !record.status().is_success())
            }) else {
                return Ok(Vec::new());
            };
            return Ok(if self.step_is_claimable_at(step.id(), definition, now)? {
                vec![step.id()]
            } else {
                Vec::new()
            });
        }

        let capacity = usize::from(definition.max_parallel().effective_with(host_ceiling).get());
        let live = steps
            .iter()
            .filter(|step| {
                self.step_records
                    .get(step.id())
                    .is_some_and(|record| record.has_live_lease_at(now))
            })
            .count();
        if live >= capacity {
            return Ok(Vec::new());
        }
        steps
            .into_iter()
            .filter_map(
                |step| match self.step_is_claimable_at(step.id(), definition, now) {
                    Ok(true) => Some(Ok(step.id())),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                },
            )
            .collect()
    }

    fn step_is_claimable_at(
        &self,
        step_id: &StepId,
        definition: &CeremonyDefinition,
        now: OffsetDateTime,
    ) -> Result<bool, DomainError> {
        let step = definition.step(step_id).ok_or(DomainError::NotFound {
            what: "ceremony_instance.step",
        })?;
        let record = self
            .step_records
            .get(step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if !record.can_be_started_at(now) {
            return Ok(false);
        }
        if self.resolve_step_role(definition, step_id, None).is_err() {
            return Ok(false);
        }
        let attempt = if matches!(record.status(), StepStatus::Failed | StepStatus::InProgress) {
            record.attempt().next()?
        } else {
            StepAttempt::new(record.attempt().get())?
        };
        Ok(step.retry_policy().allows_attempt(attempt))
    }

    #[must_use]
    pub fn has_live_step_leases_at(
        &self,
        definition: &CeremonyDefinition,
        now: OffsetDateTime,
    ) -> bool {
        definition.steps_for_state(&self.current_state).any(|step| {
            self.step_records
                .get(step.id())
                .is_some_and(|record| record.has_live_lease_at(now))
        })
    }
}
