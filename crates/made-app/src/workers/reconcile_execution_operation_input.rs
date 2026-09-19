use made_core::value_objects::{
    ArtifactRef, ExecutionConnectorId, ExecutionOperationId, ExecutionRequestDigest,
    ExternalOperationId, StepClaimFence, StepResult,
};
use time::OffsetDateTime;

/// Operator-backed terminal observation for one previously persisted intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileExecutionOperationInput {
    pub operation_id: ExecutionOperationId,
    pub request_digest: ExecutionRequestDigest,
    pub producer_claim_fence: StepClaimFence,
    pub connector_id: ExecutionConnectorId,
    pub external_operation_id: Option<ExternalOperationId>,
    pub result: StepResult,
    pub evidence: Vec<ArtifactRef>,
    pub observed_at: OffsetDateTime,
}
