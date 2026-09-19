use async_trait::async_trait;

use super::{LocalExecutionError, LocalExecutionReceipt, LocalExecutionRequest};

/// Port exposed by the trusted-local execution boundary.
#[async_trait]
pub trait LocalExecutionPort: Send + Sync {
    async fn execute(
        &self,
        request: LocalExecutionRequest,
    ) -> Result<LocalExecutionReceipt, LocalExecutionError>;
}
