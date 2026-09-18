use made_core::value_objects::{ExecutionOperation, ExecutionReceipt};

/// One operation root whose receipt link is not yet present in its ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecoveryItem {
    operation: ExecutionOperation,
    receipt: Option<ExecutionReceipt>,
}

impl ExecutionRecoveryItem {
    #[must_use]
    pub const fn new(operation: ExecutionOperation, receipt: Option<ExecutionReceipt>) -> Self {
        Self { operation, receipt }
    }

    #[must_use]
    pub const fn operation(&self) -> &ExecutionOperation {
        &self.operation
    }

    #[must_use]
    pub const fn receipt(&self) -> Option<&ExecutionReceipt> {
        self.receipt.as_ref()
    }
}
