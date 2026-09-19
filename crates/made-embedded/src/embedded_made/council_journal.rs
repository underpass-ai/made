use super::EmbeddedMade;
use made_core::entities::CouncilJournalRecord;
use made_core::error::DomainError;
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalPageLimit, CouncilJournalPosition,
    DurationMs,
};

impl EmbeddedMade {
    pub async fn read_council_events(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        self.councils.journal.read(after, limit).await
    }
    pub async fn get_council_event_cursor(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        self.councils.journal.position(consumer).await
    }
    pub async fn lease_council_events(
        &self,
        consumer: &CouncilJournalConsumer,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError> {
        self.councils.journal.lease(consumer, duration).await
    }
    pub async fn acknowledge_council_events(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
    ) -> Result<(), DomainError> {
        self.councils.journal.acknowledge(lease, through).await
    }
    pub async fn release_council_events(
        &self,
        lease: &CouncilJournalLease,
    ) -> Result<(), DomainError> {
        self.councils.journal.release(lease).await
    }
}
