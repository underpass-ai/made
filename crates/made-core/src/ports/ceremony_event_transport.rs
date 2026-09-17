use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::PositionedRecord;

/// Delivers one globally positioned ceremony record to an external sink.
#[async_trait]
pub trait CeremonyEventTransportPort: Send + Sync {
    /// Return only after the transport has accepted the record.
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError>;
}
