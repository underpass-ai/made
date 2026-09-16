use crate::entities::ceremony_commands::{ApplyStepResult, StartStep};
use crate::entities::CeremonyCommand;

use super::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, DomainError, OffsetDateTime, RoleId,
    StepAttempt, StepId, StepLease, StepResult,
};

/// The step mutators, as wrappers over [`CeremonyInstance::decide`]
/// and [`CeremonyInstance::apply`]: each decides its command, folds
/// the events, and returns what its callers always got.
impl CeremonyInstance {
    pub fn start_step_as(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        self.start_step_with(definition, Some(role_id.clone()), step_id, lease, now)
    }

    pub fn start_step(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        self.start_step_with(definition, None, step_id, lease, now)
    }

    fn start_step_with(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: Option<RoleId>,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        let command = CeremonyCommand::StartStep(StartStep {
            role_id,
            step_id: step_id.clone(),
            lease,
            now,
        });
        let events = self.decide(&command, definition)?;
        let attempt = events
            .iter()
            .find_map(|event| match event {
                CeremonyEvent::StepStarted(started) => Some(started.attempt),
                _ => None,
            })
            .ok_or(DomainError::InvariantViolated {
                reason: "starting a step decides a step start",
            })?;
        self.apply_all(&events);
        Ok(attempt)
    }

    pub fn apply_step_result(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        result: StepResult,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step_id.clone(),
            result,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }
}
