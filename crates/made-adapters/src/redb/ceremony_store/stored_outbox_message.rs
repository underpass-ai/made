use made_core::value_objects::{OutboxAttempt, OutboxMessage, OutboxQuarantineReason};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A committed message and everything the store knows about getting it
/// out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredOutboxMessage {
    pub(super) message: OutboxMessage,
    pub(super) attempt: OutboxAttempt,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub(super) claimed_until: Option<OffsetDateTime>,
    pub(super) delivered: bool,
    pub(super) quarantine: Option<OutboxQuarantineReason>,
}

impl StoredOutboxMessage {
    pub(super) fn enqueued(message: OutboxMessage) -> Self {
        Self {
            message,
            attempt: OutboxAttempt::NONE,
            claimed_until: None,
            delivered: false,
            quarantine: None,
        }
    }

    pub(super) fn is_claimable(&self, now: OffsetDateTime) -> bool {
        !self.delivered
            && self.quarantine.is_none()
            && self.claimed_until.is_none_or(|until| until <= now)
    }
}
