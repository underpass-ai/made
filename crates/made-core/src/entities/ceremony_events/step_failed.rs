use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{RoleId, StepAttempt, StepId, StepIteration, StepResult};

/// A step ended in failure.
///
/// The result carries the error message. A failure never reopens an
/// iteration, so unlike [`super::StepCompleted`] there is nothing to
/// say about what comes next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepFailed {
    pub step_id: StepId,
    pub iteration: StepIteration,
    pub attempt: StepAttempt,
    pub result: StepResult,
    pub finished_by: RoleId,
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at: OffsetDateTime,
}
