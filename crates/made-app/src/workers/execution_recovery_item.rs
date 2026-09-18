use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionReceipt, StepClaimFence,
};

/// One operation root whose receipt link is not yet present in its ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecoveryItem {
    operation: ExecutionOperation,
    intents: Vec<ExecutionIntent>,
    receipt: Option<ExecutionReceipt>,
    current_claim_fence: Option<StepClaimFence>,
}

impl ExecutionRecoveryItem {
    #[must_use]
    pub const fn new(
        operation: ExecutionOperation,
        intents: Vec<ExecutionIntent>,
        receipt: Option<ExecutionReceipt>,
        current_claim_fence: Option<StepClaimFence>,
    ) -> Self {
        Self {
            operation,
            intents,
            receipt,
            current_claim_fence,
        }
    }

    #[must_use]
    pub const fn operation(&self) -> &ExecutionOperation {
        &self.operation
    }

    #[must_use]
    pub const fn receipt(&self) -> Option<&ExecutionReceipt> {
        self.receipt.as_ref()
    }

    #[must_use]
    pub fn intents(&self) -> &[ExecutionIntent] {
        &self.intents
    }

    #[must_use]
    pub const fn current_claim_fence(&self) -> Option<&StepClaimFence> {
        self.current_claim_fence.as_ref()
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        ExecutionOperation,
        Vec<ExecutionIntent>,
        Option<ExecutionReceipt>,
        Option<StepClaimFence>,
    ) {
        (
            self.operation,
            self.intents,
            self.receipt,
            self.current_claim_fence,
        )
    }
}
