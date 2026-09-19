use crate::entities::{CouncilJournalEvent, CouncilJournalRecord};
use crate::error::DomainError;
use crate::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalPageLimit, CouncilJournalPosition,
    DurationMs,
};
use async_trait::async_trait;
use time::OffsetDateTime;

/// Council delivery is independent of ceremony delivery. Immutable publication
/// ids deduplicate append; external consumers deduplicate effects by record
/// identity because a crash before acknowledgement causes redelivery.
#[async_trait]
pub trait CouncilJournalPort: Send + Sync {
    /// Publication events require their original stable EventId. Reusing it
    /// with different content refuses without changing storage.
    async fn publish(
        &self,
        event: CouncilJournalEvent,
    ) -> Result<CouncilJournalRecord, DomainError>;
    async fn read(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError>;
    async fn position(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError>;
    async fn lease(
        &self,
        consumer: &CouncilJournalConsumer,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError>;
    /// Atomically advance and release. Expired, replaced, fabricated or already
    /// released leases cannot advance or interfere with a subsequent worker.
    async fn acknowledge(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;
    async fn release(
        &self,
        lease: &CouncilJournalLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;
}
