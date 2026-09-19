use made_core::value_objects::{CeremonyId, ExecutionConnectorId, ExecutionOperationId, StepId};

/// Discovery is not a claim: no lease, fence or budget is consumed here.
#[derive(Debug, Clone)]
pub(super) struct CeremonyWorkCandidate {
    pub ceremony: CeremonyId,
    pub step: StepId,
    pub root: CeremonyId,
    pub operation: ExecutionOperationId,
    pub provider: ExecutionConnectorId,
}
