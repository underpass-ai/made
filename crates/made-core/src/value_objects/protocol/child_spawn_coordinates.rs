use serde::{Deserialize, Serialize};

use super::{StateIteration, StateVisit, StepId, StepIteration};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChildSpawnCoordinates {
    step_id: StepId,
    state_visit: StateVisit,
    state_iteration: StateIteration,
    step_iteration: StepIteration,
}

impl ChildSpawnCoordinates {
    #[must_use]
    pub fn new(
        step_id: StepId,
        state_visit: StateVisit,
        state_iteration: StateIteration,
        step_iteration: StepIteration,
    ) -> Self {
        Self {
            step_id,
            state_visit,
            state_iteration,
            step_iteration,
        }
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub const fn state_visit(&self) -> StateVisit {
        self.state_visit
    }
    #[must_use]
    pub const fn state_iteration(&self) -> StateIteration {
        self.state_iteration
    }
    #[must_use]
    pub const fn step_iteration(&self) -> StepIteration {
        self.step_iteration
    }
}
