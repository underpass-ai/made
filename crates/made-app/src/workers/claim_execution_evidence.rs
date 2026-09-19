use made_core::value_objects::{
    ExecutionReceiptId, ExecutionRecoveryCapability, ExternalOperationId, StepClaimFence,
    StepStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimExecutionEvidence {
    pub intent_recorded: bool,
    pub receipt_id: Option<ExecutionReceiptId>,
    pub receipt_producer_fence: Option<StepClaimFence>,
    pub receipt_status: Option<StepStatus>,
    pub external_operation_id: Option<ExternalOperationId>,
    pub recovery_capability: Option<ExecutionRecoveryCapability>,
    pub receipt_applied: bool,
    pub reconciliation_required: bool,
}
