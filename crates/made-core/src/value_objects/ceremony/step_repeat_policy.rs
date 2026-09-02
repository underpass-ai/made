use serde::{Deserialize, Serialize};

use super::{RepeatUntilCondition, StepIteration, StepOutput};

/// Bounded semantic repetition for a successful ceremony step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRepeatPolicy {
    until: RepeatUntilCondition,
    max_iterations: StepIteration,
}

impl StepRepeatPolicy {
    #[must_use]
    pub fn new(until: RepeatUntilCondition, max_iterations: StepIteration) -> Self {
        Self {
            until,
            max_iterations,
        }
    }

    #[must_use]
    pub fn until(&self) -> &RepeatUntilCondition {
        &self.until
    }

    #[must_use]
    pub fn max_iterations(&self) -> StepIteration {
        self.max_iterations
    }

    #[must_use]
    pub fn is_satisfied(&self, output: &StepOutput) -> bool {
        self.until.is_satisfied(output)
    }

    #[must_use]
    pub fn permits_another_iteration(&self, current: StepIteration) -> bool {
        current < self.max_iterations
    }
}
