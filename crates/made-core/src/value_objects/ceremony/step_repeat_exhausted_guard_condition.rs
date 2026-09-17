use serde::{Deserialize, Serialize};

use super::StepId;

/// Reference to the repeating step whose cap may route one transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRepeatExhaustedGuardCondition {
    step_id: StepId,
}

impl StepRepeatExhaustedGuardCondition {
    #[must_use]
    pub fn new(step_id: StepId) -> Self {
        Self { step_id }
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
}
