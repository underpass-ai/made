use made_core::value_objects::{
    AuditActorKind, CeremonyId, ExecutionOperationId, StepClaimFence, StepId,
};

/// A current claim asking to consume one persisted terminal receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteExecutionReceiptInput {
    pub ceremony_id: CeremonyId,
    pub step_id: StepId,
    pub operation_id: ExecutionOperationId,
    pub claim_fence: StepClaimFence,
    pub actor_kind: AuditActorKind,
}
