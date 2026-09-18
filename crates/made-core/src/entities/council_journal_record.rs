use crate::entities::CouncilJournalEvent;
use crate::value_objects::CouncilJournalPosition;
use serde::{Deserialize, Serialize};

/// A durable council fact sealed at one position, returned unchanged on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouncilJournalRecord {
    position: CouncilJournalPosition,
    event: CouncilJournalEvent,
}
impl CouncilJournalRecord {
    #[must_use]
    pub fn new(position: CouncilJournalPosition, event: CouncilJournalEvent) -> Self {
        Self { position, event }
    }
    #[must_use]
    pub fn position(&self) -> CouncilJournalPosition {
        self.position
    }
    #[must_use]
    pub fn event(&self) -> &CouncilJournalEvent {
        &self.event
    }
}
