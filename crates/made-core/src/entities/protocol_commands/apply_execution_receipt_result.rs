use time::OffsetDateTime;

use crate::value_objects::{ExecutionReceiptLink, StepClaimFence, StepId, StepResult};

/// Atomically link one immutable receipt and file the result it observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyExecutionReceiptResult {
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub receipt_link: ExecutionReceiptLink,
    pub result: StepResult,
    pub now: OffsetDateTime,
}
