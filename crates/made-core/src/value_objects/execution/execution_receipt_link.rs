use serde::{Deserialize, Serialize};

use super::{ExecutionOperationId, ExecutionReceiptId, ExecutionReceiptLinkKind};
use crate::error::DomainError;
use crate::value_objects::ceremony::StepClaimFence;

/// Immutable reference sealed beside the step result that consumed a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceiptLink {
    receipt_id: ExecutionReceiptId,
    operation_id: ExecutionOperationId,
    producer_claim_fence: StepClaimFence,
    applied_claim_fence: StepClaimFence,
    kind: ExecutionReceiptLinkKind,
}

impl ExecutionReceiptLink {
    pub fn new(
        receipt_id: ExecutionReceiptId,
        operation_id: ExecutionOperationId,
        producer_claim_fence: StepClaimFence,
        applied_claim_fence: StepClaimFence,
        kind: ExecutionReceiptLinkKind,
    ) -> Result<Self, DomainError> {
        let link = Self {
            receipt_id,
            operation_id,
            producer_claim_fence,
            applied_claim_fence,
            kind,
        };
        link.validate()?;
        Ok(link)
    }

    /// Re-check identity and fence semantics after deserialization.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.receipt_id != ExecutionReceiptId::for_operation(&self.operation_id) {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt link id does not match its operation",
            });
        }
        let same = self.producer_claim_fence == self.applied_claim_fence;
        if same != matches!(self.kind, ExecutionReceiptLinkKind::Direct) {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt link kind does not match its claim fences",
            });
        }
        Ok(())
    }

    #[must_use]
    pub const fn receipt_id(&self) -> &ExecutionReceiptId {
        &self.receipt_id
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }

    #[must_use]
    pub const fn applied_claim_fence(&self) -> &StepClaimFence {
        &self.applied_claim_fence
    }

    #[must_use]
    pub const fn kind(&self) -> ExecutionReceiptLinkKind {
        self.kind
    }
}
