use super::{WorkerAuthorizationError, WorkerAuthorizationTarget};
use async_trait::async_trait;
use made_core::value_objects::AuthorizedOperation;

/// Supplies a current policy decision for worker work on one ceremony.
#[async_trait]
pub trait WorkerAuthorizationPort: Send + Sync {
    async fn authorize(
        &self,
        target: &WorkerAuthorizationTarget,
    ) -> Result<AuthorizedOperation, WorkerAuthorizationError>;
}
