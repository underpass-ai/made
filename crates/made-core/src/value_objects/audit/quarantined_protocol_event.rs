use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    GlobalPosition, ProtocolEventConsumer, ProtocolEventCursorAttempt,
    ProtocolEventQuarantineReason,
};

/// One global-feed event a consumer deliberately skipped after retries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedProtocolEvent {
    consumer: ProtocolEventConsumer,
    position: GlobalPosition,
    attempts: ProtocolEventCursorAttempt,
    reason: ProtocolEventQuarantineReason,
    #[serde(with = "time::serde::rfc3339")]
    quarantined_at: OffsetDateTime,
}

impl QuarantinedProtocolEvent {
    #[must_use]
    pub fn new(
        consumer: ProtocolEventConsumer,
        position: GlobalPosition,
        attempts: ProtocolEventCursorAttempt,
        reason: ProtocolEventQuarantineReason,
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
    pub fn consumer(&self) -> &ProtocolEventConsumer {
        &self.consumer
    }

    #[must_use]
    pub const fn position(&self) -> GlobalPosition {
        self.position
    }

    #[must_use]
    pub const fn attempts(&self) -> ProtocolEventCursorAttempt {
        self.attempts
    }

    #[must_use]
    pub fn reason(&self) -> &ProtocolEventQuarantineReason {
        &self.reason
    }

    #[must_use]
    pub const fn quarantined_at(&self) -> OffsetDateTime {
        self.quarantined_at
    }
}
