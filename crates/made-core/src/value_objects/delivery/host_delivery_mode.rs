use std::fmt;

use serde::{Deserialize, Serialize};

/// How a delivery is expected to reach its destination.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HostDeliveryMode {
    /// The host asks; the ledger hands out an exclusive, expiring lease.
    #[default]
    PullLease,
    /// The engine wakes the host through its activation adapter.
    Activation,
}

impl HostDeliveryMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PullLease => "pull_lease",
            Self::Activation => "activation",
        }
    }
}

impl fmt::Display for HostDeliveryMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
