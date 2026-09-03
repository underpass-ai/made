use made_core::entities::{Deliberation, Proposal};
use made_core::value_objects::ExecutionOutcome;

/// Deliberation result together with the executor outcome.
#[derive(Debug, Clone)]
pub struct OrchestrateOutput {
    pub deliberation: Deliberation,
    pub winner: Proposal,
    pub execution: ExecutionOutcome,
}
