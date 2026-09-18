use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::ExecutionReceiptStorePort;
use made_core::value_objects::{ExecutionRecoveryCursor, ExecutionRecoveryPageLimit};

use super::{ExecutionRecoveryItem, ExecutionRecoveryItemsPage};
use crate::services::SessionStream;

/// Page operation roots and retain every one without an applied receipt link.
pub struct InspectExecutionRecoveryUseCase {
    stream: Arc<SessionStream>,
    receipts: Arc<dyn ExecutionReceiptStorePort>,
}

impl std::fmt::Debug for InspectExecutionRecoveryUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InspectExecutionRecoveryUseCase")
            .finish_non_exhaustive()
    }
}

impl InspectExecutionRecoveryUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>, receipts: Arc<dyn ExecutionReceiptStorePort>) -> Self {
        Self { stream, receipts }
    }

    pub async fn execute(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
        let page = self.receipts.recoverable(after, limit).await?;
        let (operations, next_cursor) = page.into_parts();
        let mut items = Vec::with_capacity(operations.len());
        for operation in operations {
            let session = self.stream.load(operation.ceremony_id()).await?;
            if session
                .instance
                .execution_receipt_link(operation.operation_id())
                .is_some()
            {
                continue;
            }
            let receipt = self.receipts.receipt(operation.operation_id()).await?;
            items.push(ExecutionRecoveryItem::new(operation, receipt));
        }
        Ok(ExecutionRecoveryItemsPage::new(items, next_cursor))
    }
}
