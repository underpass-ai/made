use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{StateId, StateIteration, StepId};

/// Every step in a repeated state was reopened for another complete pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateIterationStarted {
    pub state_id: StateId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_visit: Option<crate::value_objects::StateVisit>,
    pub state_iteration: StateIteration,
    pub step_ids: Vec<StepId>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

impl StateIterationStarted {
    #[must_use]
    pub fn state_visit(&self) -> crate::value_objects::StateVisit {
        self.state_visit.unwrap_or_default()
    }
}
