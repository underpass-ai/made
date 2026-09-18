use serde::{Deserialize, Serialize};

use super::{StateIteration, StateRepeatUntilCondition, StepExecutionRecord};

/// Bounded repetition of every step in one ceremony state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRepeatPolicy {
    max_iterations: StateIteration,
    until: StateRepeatUntilCondition,
}

impl StateRepeatPolicy {
    #[must_use]
    pub fn new(max_iterations: StateIteration, until: StateRepeatUntilCondition) -> Self {
        Self {
            max_iterations,
            until,
        }
    }

    #[must_use]
    pub const fn max_iterations(&self) -> StateIteration {
        self.max_iterations
    }

    #[must_use]
    pub const fn until(&self) -> &StateRepeatUntilCondition {
        &self.until
    }

    #[must_use]
    pub fn is_satisfied(&self, record: &StepExecutionRecord) -> bool {
        self.until.is_satisfied(record)
    }

    #[must_use]
    pub fn permits_another_iteration(&self, current: StateIteration) -> bool {
        current < self.max_iterations
    }
}
