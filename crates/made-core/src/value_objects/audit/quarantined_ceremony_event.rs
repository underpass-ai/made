use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventQuarantineReason,
    GlobalPosition,
};

/// One global-feed event a consumer deliberately skipped after retries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedCeremonyEvent {
    consumer: CeremonyEventConsumer,
    position: GlobalPosition,
    attempts: CeremonyEventCursorAttempt,
    reason: CeremonyEventQuarantineReason,
    #[serde(with = "time::serde::rfc3339")]
    quarantined_at: OffsetDateTime,
}

impl QuarantinedCeremonyEvent {
    #[must_use]
    pub fn new(
        consumer: CeremonyEventConsumer,
        position: GlobalPosition,
        attempts: CeremonyEventCursorAttempt,
        reason: CeremonyEventQuarantineReason,
        quarantined_at: OffsetDateTime,
    ) -> Self {
        Self {
            consumer,
            position,
            attempts,
            reason,
            quarantined_at,
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &CeremonyEventConsumer {
        &self.consumer
    }

    #[must_use]
    pub const fn position(&self) -> GlobalPosition {
        self.position
    }

    #[must_use]
    pub const fn attempts(&self) -> CeremonyEventCursorAttempt {
        self.attempts
    }

    #[must_use]
    pub fn reason(&self) -> &CeremonyEventQuarantineReason {
        &self.reason
    }

    #[must_use]
    pub const fn quarantined_at(&self) -> OffsetDateTime {
        self.quarantined_at
    }
}
