use super::{
    RoleId, StateIteration, StateVisit, StepAttempt, StepClaimFence, StepId, StepIteration,
    StepResult,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LateStepResult {
    step_id: StepId,
    state_visit: StateVisit,
    state_iteration: StateIteration,
    step_iteration: StepIteration,
    attempt: StepAttempt,
    claim_fence: StepClaimFence,
    result: StepResult,
    finished_by: RoleId,
}
impl LateStepResult {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        step_id: StepId,
        state_visit: StateVisit,
        state_iteration: StateIteration,
        step_iteration: StepIteration,
        attempt: StepAttempt,
        claim_fence: StepClaimFence,
        result: StepResult,
        finished_by: RoleId,
    ) -> Self {
        Self {
            step_id,
            state_visit,
            state_iteration,
            step_iteration,
            attempt,
            claim_fence,
            result,
            finished_by,
        }
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub fn state_visit(&self) -> StateVisit {
        self.state_visit
    }
    #[must_use]
    pub fn state_iteration(&self) -> StateIteration {
        self.state_iteration
    }
    #[must_use]
    pub fn step_iteration(&self) -> StepIteration {
        self.step_iteration
    }
    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }
    #[must_use]
    pub fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }
    #[must_use]
    pub fn result(&self) -> &StepResult {
        &self.result
    }
    #[must_use]
    pub fn finished_by(&self) -> &RoleId {
        &self.finished_by
    }
}
