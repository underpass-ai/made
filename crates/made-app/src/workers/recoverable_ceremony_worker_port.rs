use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ExecutionCancellation;

use super::{
    ExecuteCeremonyOperationInput, ExecutionRecoveryItem, RecoverableCeremonyWorkerOutcome,
};

/// One accepted or recovered unit that a bounded host may schedule.
#[async_trait]
pub trait RecoverableCeremonyWorkerPort: Send + Sync {
    async fn execute_claim(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError>;

    async fn execute_claim_cancellable(
        &self,
        input: ExecuteCeremonyOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
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
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError>;

    async fn recover_cancellable(
        &self,
        item: ExecutionRecoveryItem,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        cancellation
            .run(self.recover(item))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "worker recovery authority was cancelled",
            })?
    }
}
