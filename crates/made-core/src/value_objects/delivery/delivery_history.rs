use serde::{Deserialize, Serialize};

use super::DeliveryHistoryEntry;

/// How many transitions one delivery keeps.
///
/// Bounded because the history is stored inline with the record and a
/// delivery that a busy host keeps taking and dropping would otherwise
/// grow its own row without limit. The oldest entries go first: what a
/// delivery has been doing lately is what an operator is looking at.
const MAX_ENTRIES: usize = 32;

/// The recent states of one delivery, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeliveryHistory(Vec<DeliveryHistoryEntry>);

impl DeliveryHistory {
    /// How many transitions are kept before the oldest is forgotten.
    pub const CAPACITY: usize = MAX_ENTRIES;

    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Record one more transition, forgetting the oldest if full.
    #[must_use]
    pub fn recording(mut self, entry: DeliveryHistoryEntry) -> Self {
        if self.0.len() == MAX_ENTRIES {
            self.0.remove(0);
        }
        self.0.push(entry);
        self
    }

    #[must_use]
    pub fn entries(&self) -> &[DeliveryHistoryEntry] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The most recent transition, if there is one.
    #[must_use]
    pub fn last(&self) -> Option<DeliveryHistoryEntry> {
        self.0.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::HostDeliveryStateKind;
    use time::OffsetDateTime;

    #[test]
    fn a_busy_delivery_keeps_its_last_thirty_two_transitions() {
        let mut history = DeliveryHistory::new();
        for _ in 0..40 {
            history = history.recording(DeliveryHistoryEntry::new(
                HostDeliveryStateKind::Queued,
                OffsetDateTime::UNIX_EPOCH,
            ));
        }
        assert_eq!(history.len(), DeliveryHistory::CAPACITY);
        history = history.recording(DeliveryHistoryEntry::new(
            HostDeliveryStateKind::Processed,
            OffsetDateTime::UNIX_EPOCH,
        ));
        assert_eq!(
            history.last().unwrap().state_kind(),
            HostDeliveryStateKind::Processed
        );
        assert_eq!(history.len(), DeliveryHistory::CAPACITY);
    }
}
