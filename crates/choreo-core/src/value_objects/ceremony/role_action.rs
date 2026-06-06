use serde::{Deserialize, Serialize};

use super::{StepId, TransitionTrigger};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RoleAction {
    Step(StepId),
    Transition(TransitionTrigger),
}

impl RoleAction {
    #[must_use]
    pub fn step(step_id: StepId) -> Self {
        Self::Step(step_id)
    }

    #[must_use]
    pub fn transition(trigger: TransitionTrigger) -> Self {
        Self::Transition(trigger)
    }

    #[must_use]
    pub fn step_id(&self) -> Option<&StepId> {
        match self {
            Self::Step(step_id) => Some(step_id),
            Self::Transition(_) => None,
        }
    }

    #[must_use]
    pub fn transition_trigger(&self) -> Option<&TransitionTrigger> {
        match self {
            Self::Step(_) => None,
            Self::Transition(trigger) => Some(trigger),
        }
    }
}
