use super::EmbeddedMade;
use made_core::entities::CouncilJournalRecord;
use made_core::error::DomainError;
use made_core::value_objects::{
    AuthorizationAction, CouncilJournalConsumer, CouncilJournalLease, CouncilJournalPageLimit,
    CouncilJournalPosition, DurationMs,
};

impl EmbeddedMade {
    pub async fn read_council_events(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ReadCouncilEvents)?;
        self.councils.journal.read(after, limit).await
    }
    pub async fn get_council_event_cursor(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetCouncilEventCursor)?;
        self.councils.journal.position(consumer).await
    }
    pub async fn lease_council_events(
        &self,
        consumer: &CouncilJournalConsumer,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::LeaseCouncilEvents)?;
        self.councils.journal.lease(consumer, duration).await
    }
    pub async fn acknowledge_council_events(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
    ) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::AcknowledgeCouncilEvents)?;
        self.councils.journal.acknowledge(lease, through).await
    }
    pub async fn release_council_events(
        &self,
        lease: &CouncilJournalLease,
    ) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ReleaseCouncilEvents)?;
        self.councils.journal.release(lease).await
    }
}
