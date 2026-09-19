use async_trait::async_trait;
use made_core::error::DomainError;

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

    async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError>;
}
