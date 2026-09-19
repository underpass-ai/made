use super::{WorkerCapacityRenewalGuard, WorkerCapacityRequest};
use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::value_objects::{ExecutionOperationId, LeaseOwnerId, StepClaimFence};
use time::OffsetDateTime;

/// Shared admission reservation. Implementations fail closed on unknown journal state.
#[async_trait]
pub trait WorkerCapacityPort: Send + Sync {
    async fn reserve(
        &self,
        request: &WorkerCapacityRequest,
        now: OffsetDateTime,
    ) -> Result<bool, DomainError>;
    async fn bind(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
        fence: &StepClaimFence,
    ) -> Result<(), DomainError>;
    async fn lock_renewal(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
        fence: &StepClaimFence,
    ) -> Result<Box<dyn WorkerCapacityRenewalGuard>, DomainError>;
    async fn release(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
    ) -> Result<(), DomainError>;
}
