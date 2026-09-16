use time::OffsetDateTime;

use crate::value_objects::{StepId, StepResult};

/// File the outcome of the step currently running.
///
/// Whoever finished it is the seat the definition assigns to the
/// step, so the command does not name one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyStepResult {
    pub step_id: StepId,
    pub result: StepResult,
    pub now: OffsetDateTime,
}
