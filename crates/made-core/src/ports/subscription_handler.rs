use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::DomainEvent;

/// Handles already-deserialized messages at the domain boundary.
#[async_trait]
pub trait SubscriptionHandler<E: DomainEvent>: Send + Sync {
    async fn handle(&self, event: E) -> Result<(), DomainError>;
}
