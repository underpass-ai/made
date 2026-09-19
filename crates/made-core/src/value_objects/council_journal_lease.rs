use super::{CouncilJournalConsumer, CouncilJournalLeaseId, CouncilJournalPosition};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// An exclusive council-consumer lease. Its identity is issued by the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CouncilJournalLease {
    consumer: CouncilJournalConsumer,
    id: CouncilJournalLeaseId,
    acknowledged_through: Option<CouncilJournalPosition>,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}
impl CouncilJournalLease {
    #[must_use]
    pub fn new(
        consumer: CouncilJournalConsumer,
        id: CouncilJournalLeaseId,
        acknowledged_through: Option<CouncilJournalPosition>,
        expires_at: OffsetDateTime,
    ) -> Self {
        Self {
            consumer,
            id,
            acknowledged_through,
            expires_at,
        }
    }
    #[must_use]
    pub fn consumer(&self) -> &CouncilJournalConsumer {
        &self.consumer
    }
    #[must_use]
    pub fn id(&self) -> &CouncilJournalLeaseId {
        &self.id
    }
    #[must_use]
    pub fn acknowledged_through(&self) -> Option<CouncilJournalPosition> {
        self.acknowledged_through
    }
    #[must_use]
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
}
