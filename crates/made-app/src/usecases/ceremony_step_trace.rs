use made_core::value_objects::{RoleId, StateId, StepAttempt, StepId, StepOutput, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyStepTrace {
    state_id: StateId,
    step_id: StepId,
    role_id: RoleId,
    attempt: StepAttempt,
    status: StepStatus,
    output: StepOutput,
}

impl CeremonyStepTrace {
    #[must_use]
    pub fn new(
        state_id: StateId,
        step_id: StepId,
        role_id: RoleId,
        attempt: StepAttempt,
        status: StepStatus,
        output: StepOutput,
    ) -> Self {
        Self {
            state_id,
            step_id,
            role_id,
            attempt,
            status,
            output,
        }
    }

    #[must_use]
    pub fn state_id(&self) -> &StateId {
        &self.state_id
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }

    #[must_use]
    pub fn status(&self) -> StepStatus {
        self.status
    }

    #[must_use]
    pub fn output(&self) -> &StepOutput {
        &self.output
    }
}
