use made_core::entities::RankedOutcome;
use made_core::value_objects::{DurationMs, TaskId, ValidationMode};

/// Validated council decision and all ranked candidates.
#[derive(Debug, Clone)]
pub struct RunCouncilDecisionOutput {
    pub task_id: TaskId,
    pub winner: RankedOutcome,
    pub candidates: Vec<RankedOutcome>,
    pub validation_mode: ValidationMode,
    pub passed: bool,
    pub duration_ms: DurationMs,
}
