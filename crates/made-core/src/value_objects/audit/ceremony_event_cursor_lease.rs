use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventCursorLeaseId, GlobalPosition,
};

/// Exclusive, expiring right to process one named cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyEventCursorLease {
    consumer: CeremonyEventConsumer,
    lease_id: CeremonyEventCursorLeaseId,
    acknowledged_through: Option<GlobalPosition>,
    attempt: CeremonyEventCursorAttempt,
    #[serde(with = "time::serde::rfc3339")]
    leased_until: OffsetDateTime,
}

impl CeremonyEventCursorLease {
    #[must_use]
    pub fn new(
        consumer: CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        acknowledged_through: Option<GlobalPosition>,
        attempt: CeremonyEventCursorAttempt,
        leased_until: OffsetDateTime,
    ) -> Self {
        Self {
            consumer,
            lease_id,
            acknowledged_through,
            attempt,
            leased_until,
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &CeremonyEventConsumer {
        &self.consumer
    }

    #[must_use]
    pub fn lease_id(&self) -> &CeremonyEventCursorLeaseId {
        &self.lease_id
    }

    #[must_use]
    pub const fn acknowledged_through(&self) -> Option<GlobalPosition> {
        self.acknowledged_through
    }

    #[must_use]
    pub fn next_position(&self) -> GlobalPosition {
        match self.acknowledged_through {
            Some(position) => position.next(),
            None => GlobalPosition::FIRST,
        }
    }

    #[must_use]
    pub const fn attempt(&self) -> CeremonyEventCursorAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn leased_until(&self) -> OffsetDateTime {
        self.leased_until
    }
}
