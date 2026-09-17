use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{StateId, StateIteration, StepId};

/// Every step in a repeated state was reopened for another complete pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateIterationStarted {
    pub state_id: StateId,
    pub state_iteration: StateIteration,
    pub step_ids: Vec<StepId>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}
