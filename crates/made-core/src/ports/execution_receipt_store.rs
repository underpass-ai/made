use async_trait::async_trait;

use super::{ExecutionRecoveryPage, RecordExecutionIntentOutcome, RecordExecutionReceiptOutcome};
use crate::error::DomainError;
use crate::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionOperationId, ExecutionReceipt,
    ExecutionRecoveryCursor, ExecutionRecoveryPageLimit, StepClaimFence,
};

/// Durable intents and immutable terminal receipts for recoverable workers.
#[async_trait]
pub trait ExecutionReceiptStorePort: Send + Sync {
    async fn record_intent(
        &self,
        intent: ExecutionIntent,
    ) -> Result<RecordExecutionIntentOutcome, DomainError>;

    async fn intent(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionIntent>, DomainError>;

    async fn intents(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Vec<ExecutionIntent>, DomainError>;

    async fn operation(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionOperation>, DomainError>;

    async fn receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionReceipt>, DomainError>;

    async fn record_receipt(
        &self,
        receipt: ExecutionReceipt,
    ) -> Result<RecordExecutionReceiptOutcome, DomainError>;

    async fn recoverable(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryPage, DomainError>;
}
