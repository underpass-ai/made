use made_core::value_objects::{ExecutionConnectorId, ExecutionOperationId, ProtocolId, StepId};

/// Discovery is not a claim: no lease, fence or budget is consumed here.
#[derive(Debug, Clone)]
pub(super) struct ProtocolWorkCandidate {
    pub protocol: ProtocolId,
    pub step: StepId,
    pub root: ProtocolId,
    pub operation: ExecutionOperationId,
    pub provider: ExecutionConnectorId,
}
