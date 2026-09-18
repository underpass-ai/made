use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::value_objects::{ExecutionRecoveryCursor, ExecutionRecoveryPageLimit};

use super::{ExecutionRecoveryItemsPage, InspectExecutionRecoveryUseCase};

/// Pages operations whose durable receipt has not yet been linked.
#[async_trait]
pub trait ExecutionRecoveryInspectorPort: Send + Sync {
    async fn inspect(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError>;
}

#[async_trait]
impl ExecutionRecoveryInspectorPort for InspectExecutionRecoveryUseCase {
    async fn inspect(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
        self.execute(after, limit).await
    }
}
