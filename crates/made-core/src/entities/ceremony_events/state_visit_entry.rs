use crate::value_objects::{StateVisit, StepId};
use serde::{Deserialize, Serialize};

/// Complete destination reset sealed by a transition decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateVisitEntry {
    pub state_visit: StateVisit,
    pub step_ids: Vec<StepId>,
}
