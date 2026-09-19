use made_core::entities::CouncilJournalRecord;
use made_core::error::DomainError;
use made_core::ports::{ClockPort, CouncilJournalPort};
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalPageLimit, CouncilJournalPosition,
    DurationMs,
};
use std::sync::Arc;

/// Public council consumption uses host time and the same store contract on
/// every surface. Payload timestamps cannot extend consumer ownership.
pub struct CouncilJournalService {
    journal: Arc<dyn CouncilJournalPort>,
    clock: Arc<dyn ClockPort>,
}
impl std::fmt::Debug for CouncilJournalService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CouncilJournalService")
            .finish_non_exhaustive()
    }
}
impl CouncilJournalService {
    #[must_use]
    pub fn new(journal: Arc<dyn CouncilJournalPort>, clock: Arc<dyn ClockPort>) -> Self {
        Self { journal, clock }
    }
    pub async fn read(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        self.journal.read(after, limit).await
    }
    pub async fn position(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        self.journal.position(consumer).await
    }
    pub async fn lease(
        &self,
        consumer: &CouncilJournalConsumer,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError> {
        self.journal
            .lease(consumer, self.clock.now(), duration)
            .await
    }
    pub async fn acknowledge(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
    ) -> Result<(), DomainError> {
        self.journal
            .acknowledge(lease, through, self.clock.now())
            .await
    }
    pub async fn release(&self, lease: &CouncilJournalLease) -> Result<(), DomainError> {
        self.journal.release(lease, self.clock.now()).await
    }
}
