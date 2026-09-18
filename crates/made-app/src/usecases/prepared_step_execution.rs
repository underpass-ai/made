use made_core::value_objects::{CeremonyTranscript, StepResult};

/// Work selected for one claimed step after applying its aggregation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedStepExecution {
    /// Invoke the step's configured handler with this transcript.
    Handler { transcript: CeremonyTranscript },
    /// Complete the claimed step directly, without a handler or model call.
    Deterministic { result: StepResult },
}
