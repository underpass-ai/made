use async_trait::async_trait;

use crate::error::DomainError;
use crate::value_objects::OutboxMessage;

/// Delivers a serialized message after it leaves the outbox store.
#[async_trait]
pub trait OutboxTransportPort: Send + Sync {
    async fn deliver(&self, message: &OutboxMessage) -> Result<(), DomainError>;
}
