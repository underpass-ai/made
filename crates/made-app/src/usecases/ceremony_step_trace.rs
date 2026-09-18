use made_core::value_objects::{
    RoleId, StateId, StateIteration, StateVisit, StepAttempt, StepId, StepIteration, StepOutput,
    StepStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyStepTrace {
    state_id: StateId,
    state_iteration: StateIteration,
    state_visit: StateVisit,
    step_id: StepId,
    role_id: RoleId,
    iteration: StepIteration,
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
        Self::for_coordinates(
            state_id,
            StateIteration::FIRST,
            step_id,
            role_id,
            StepIteration::FIRST,
            attempt,
            status,
            output,
        )
    }

    #[must_use]
    pub fn for_iteration(
        state_id: StateId,
        step_id: StepId,
        role_id: RoleId,
        iteration: StepIteration,
        attempt: StepAttempt,
        status: StepStatus,
        output: StepOutput,
    ) -> Self {
        Self::for_coordinates(
            state_id,
            StateIteration::FIRST,
            step_id,
            role_id,
            iteration,
            attempt,
            status,
            output,
        )
    }

    #[must_use]
    pub fn for_coordinates(
        state_id: StateId,
        state_iteration: StateIteration,
        step_id: StepId,
        role_id: RoleId,
        iteration: StepIteration,
        attempt: StepAttempt,
        status: StepStatus,
        output: StepOutput,
    ) -> Self {
        Self {
            state_id,
            state_iteration,
            state_visit: StateVisit::FIRST,
            step_id,
            role_id,
            iteration,
            attempt,
            status,
            output,
        }
    }

    #[must_use]
    pub fn iteration(&self) -> StepIteration {
        self.iteration
    }

    #[must_use]
    pub fn state_visit(&self) -> StateVisit {
        self.state_visit
    }

    #[must_use]
    pub fn with_state_visit(mut self, state_visit: StateVisit) -> Self {
        self.state_visit = state_visit;
        self
    }

    #[must_use]
    pub fn state_iteration(&self) -> StateIteration {
        self.state_iteration
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
