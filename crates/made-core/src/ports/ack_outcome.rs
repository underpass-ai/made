use crate::value_objects::{HostDeliveryObservation, HostDeliveryRecord};

/// What the ledger did with a host's report about a delivery.
///
/// The four answers are deliberately different things. A repeat of the
/// same report is the same fact arriving twice and costs nothing; a
/// different report about a delivery already acknowledged is two hosts
/// disagreeing and must not be flattened into the later one; and a
/// lease that is not the current one is a host that was replaced while
/// it was away, whose answer arrives too late to be worth anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckOutcome {
    /// Recorded.
    Acknowledged(HostDeliveryRecord),
    /// The same report, already recorded.
    AlreadyAcknowledged(HostDeliveryRecord),
    /// A different report about a delivery that is already acknowledged.
    Conflict {
        existing: Box<HostDeliveryObservation>,
    },
    /// The lease offered is not the one that holds this delivery.
    LeaseNotOwned,
}

impl AckOutcome {
    /// The record, when the call reached one.
    #[must_use]
    pub const fn record(&self) -> Option<&HostDeliveryRecord> {
        match self {
            Self::Acknowledged(record) | Self::AlreadyAcknowledged(record) => Some(record),
            Self::Conflict { .. } | Self::LeaseNotOwned => None,
        }
    }

    #[must_use]
    pub const fn is_recorded(&self) -> bool {
        matches!(self, Self::Acknowledged(_) | Self::AlreadyAcknowledged(_))
    }
}
