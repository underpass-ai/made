use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::ExecutionReceiptStorePort;
use made_core::value_objects::{ExecutionOperationId, ExecutionReceipt};

/// Read one immutable execution receipt by its stable operation identity.
pub struct GetExecutionReceiptUseCase {
    receipts: Arc<dyn ExecutionReceiptStorePort>,
}

impl std::fmt::Debug for GetExecutionReceiptUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GetExecutionReceiptUseCase")
            .finish_non_exhaustive()
    }
}

impl GetExecutionReceiptUseCase {
    #[must_use]
    pub const fn new(receipts: Arc<dyn ExecutionReceiptStorePort>) -> Self {
        Self { receipts }
    }

    pub async fn execute(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<ExecutionReceipt, DomainError> {
        self.receipts
            .receipt(operation_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "execution_receipt",
            })
    }
}
