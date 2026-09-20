use crate::value_objects::HostDeliveryRecord;

/// What a failed attempt did to a delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryFailureOutcome {
    /// Attempts remain; it is back in the queue.
    Requeued(HostDeliveryRecord),
    /// Attempts ran out; the failure is visible and stays.
    Exhausted(HostDeliveryRecord),
    /// The lease offered is not the one that holds this delivery.
    LeaseNotOwned,
}

impl DeliveryFailureOutcome {
    #[must_use]
    pub const fn record(&self) -> Option<&HostDeliveryRecord> {
        match self {
            Self::Requeued(record) | Self::Exhausted(record) => Some(record),
            Self::LeaseNotOwned => None,
        }
    }

    #[must_use]
    pub const fn is_exhausted(&self) -> bool {
        matches!(self, Self::Exhausted(_))
    }
}
