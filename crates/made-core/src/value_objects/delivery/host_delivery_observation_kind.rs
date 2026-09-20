use std::fmt;

use serde::{Deserialize, Serialize};

/// What the host said when it was handed a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostDeliveryObservationKind {
    /// The host has it. Whether it acts on it is a later fact.
    Received,
    /// The host has it and will not act on it.
    Refused,
    /// The host cannot act on this kind of item at all.
    Incapable,
    /// The host is busy; offering it again later is reasonable.
    Busy,
    /// The host did not answer within its own time.
    Timeout,
}

impl HostDeliveryObservationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Refused => "refused",
            Self::Incapable => "incapable",
            Self::Busy => "busy",
            Self::Timeout => "timeout",
        }
    }

    /// Whether offering the same item again could plausibly work.
    ///
    /// A refusal and an incapacity are answers; a busy host and a
    /// silent one are not, and only those two are worth a retry.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Busy | Self::Timeout)
    }

    /// Whether the host took the item.
    #[must_use]
    pub const fn is_received(self) -> bool {
        matches!(self, Self::Received)
    }
}

impl fmt::Display for HostDeliveryObservationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
