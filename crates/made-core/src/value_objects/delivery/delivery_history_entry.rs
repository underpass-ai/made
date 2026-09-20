use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::HostDeliveryStateKind;

/// One state a delivery passed through, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryHistoryEntry {
    state_kind: HostDeliveryStateKind,
    #[serde(with = "time::serde::rfc3339")]
    at: OffsetDateTime,
}

impl DeliveryHistoryEntry {
    #[must_use]
    pub const fn new(state_kind: HostDeliveryStateKind, at: OffsetDateTime) -> Self {
        Self { state_kind, at }
    }

    #[must_use]
    pub const fn state_kind(self) -> HostDeliveryStateKind {
        self.state_kind
    }

    #[must_use]
    pub const fn at(self) -> OffsetDateTime {
        self.at
    }
}
