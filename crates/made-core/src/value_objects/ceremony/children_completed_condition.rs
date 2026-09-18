use serde::{Deserialize, Serialize};

use super::{ChildJoin, StepId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildrenCompletedCondition {
    step_id: StepId,
    join: ChildJoin,
}

impl ChildrenCompletedCondition {
    #[must_use]
    pub fn new(step_id: StepId, join: ChildJoin) -> Self {
        Self { step_id, join }
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub const fn join(&self) -> ChildJoin {
        self.join
    }
}
