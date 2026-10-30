use made_core::value_objects::{
    AuditActorKind, ExecutionOperationId, ExecutionReceiptLinkKind, ProtocolId, StepClaimFence,
    StepId,
};

/// A current claim asking to consume one persisted terminal receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteExecutionReceiptInput {
    pub protocol_id: ProtocolId,
    pub step_id: StepId,
    pub operation_id: ExecutionOperationId,
    pub claim_fence: StepClaimFence,
    pub link_kind: ExecutionReceiptLinkKind,
    pub actor_kind: AuditActorKind,
}
