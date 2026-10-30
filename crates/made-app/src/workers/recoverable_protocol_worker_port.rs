use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ExecutionCancellation;

use super::{
    ExecuteProtocolOperationInput, ExecutionRecoveryItem, RecoverableProtocolWorkerOutcome,
};

/// One accepted or recovered unit that a bounded host may schedule.
#[async_trait]
pub trait RecoverableProtocolWorkerPort: Send + Sync {
    async fn execute_claim(
        &self,
        input: ExecuteProtocolOperationInput,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError>;

    async fn execute_claim_cancellable(
        &self,
        input: ExecuteProtocolOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        cancellation
            .run(self.execute_claim(input))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "worker authority was cancelled",
            })?
    }

    async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError>;

    async fn recover_cancellable(
        &self,
        item: ExecutionRecoveryItem,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        cancellation
            .run(self.recover(item))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "worker recovery authority was cancelled",
            })?
    }
}
