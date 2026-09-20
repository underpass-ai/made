use std::fmt;

use serde::{Deserialize, Serialize};

/// The name of a delivery's state, without what the state carries.
///
/// Its own type because two places need the name and not the payload:
/// the bounded history of what a delivery has been, and a query that
/// asks for everything currently in one state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostDeliveryStateKind {
    Queued,
    Leased,
    DeliveredToHost,
    Acknowledged,
    Processed,
    Failed,
    Expired,
    Superseded,
}

impl HostDeliveryStateKind {
    /// Every state, in the order a delivery normally passes through them.
    pub const ALL: [Self; 8] = [
        Self::Queued,
        Self::Leased,
        Self::DeliveredToHost,
        Self::Acknowledged,
        Self::Processed,
        Self::Failed,
        Self::Expired,
        Self::Superseded,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::DeliveredToHost => "delivered_to_host",
            Self::Acknowledged => "acknowledged",
            Self::Processed => "processed",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Superseded => "superseded",
        }
    }

    /// Whether nothing further will happen to a delivery in this state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Processed | Self::Failed | Self::Expired | Self::Superseded
        )
    }

    /// Whether a host could still be handed this delivery.
    #[must_use]
    pub const fn is_offerable(self) -> bool {
        matches!(self, Self::Queued | Self::DeliveredToHost)
    }
}

impl fmt::Display for HostDeliveryStateKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_are_never_offerable() {
        for kind in HostDeliveryStateKind::ALL {
            assert!(
                !(kind.is_terminal() && kind.is_offerable()),
                "{kind} is both terminal and offerable"
            );
        }
    }
}
