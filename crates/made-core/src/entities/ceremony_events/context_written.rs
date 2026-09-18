use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{ContextPatch, StateIteration, StepAttempt, StepId, StepIteration};

/// A successful step atomically promoted declared output fields into context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWritten {
    pub step_id: StepId,
    pub state_iteration: StateIteration,
    pub iteration: StepIteration,
    pub attempt: StepAttempt,
    pub patch: ContextPatch,
    #[serde(with = "time::serde::rfc3339")]
    pub written_at: OffsetDateTime,
}
