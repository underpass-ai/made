use crate::entities::ceremony_commands::{ApplyStepResult, StartStep};
use crate::entities::CeremonyCommand;
use crate::value_objects::{MaxParallel, StepClaimFence};

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
        self.start_step_with(
            definition,
            Some(role_id.clone()),
            step_id,
            lease,
            now,
            MaxParallel::SERVER_MAX,
        )
    }

    pub fn start_step(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        self.start_step_with(
            definition,
            None,
            step_id,
            lease,
            now,
            MaxParallel::SERVER_MAX,
        )
    }

    fn start_step_with(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: Option<RoleId>,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
        max_parallel_ceiling: MaxParallel,
    ) -> Result<StepAttempt, DomainError> {
        let command = CeremonyCommand::StartStep(StartStep {
            role_id,
            step_id: step_id.clone(),
            lease,
            now,
            max_parallel_ceiling,
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

    /// Capture this identity from the accepted claim before starting work.
    pub fn step_claim_fence(&self, step_id: &StepId) -> Result<StepClaimFence, DomainError> {
        let record = self.step_record(step_id).ok_or(DomainError::NotFound {
            what: "ceremony_instance.step_record",
        })?;
        StepClaimFence::for_record(self.id(), step_id, record)
    }

    pub(super) fn require_step_claim_fence(
        &self,
        step_id: &StepId,
        fence: &StepClaimFence,
    ) -> Result<(), DomainError> {
        if &self.step_claim_fence(step_id)? != fence {
            return Err(DomainError::InvariantViolated {
                reason: "step completion claim fence does not match the current claim",
            });
        }
        Ok(())
    }

    pub fn apply_step_result(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        claim_fence: StepClaimFence,
        result: StepResult,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step_id.clone(),
            claim_fence,
            result,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }
}
