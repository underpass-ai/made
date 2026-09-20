use crate::value_objects::HostDeliveryRecord;

/// What the ledger did with an offered delivery.
///
/// Idempotent by identity: the same item offered to the same
/// destination twice is one delivery, and the second offer hands back
/// what is already there rather than a second copy of it. That is what
/// lets a projector replay the feed after a restart without doubling
/// every hand-off it already made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// New: the ledger is holding it now.
    Enqueued(HostDeliveryRecord),
    /// Already held, in whatever state it had reached.
    AlreadyQueued(HostDeliveryRecord),
}

impl EnqueueOutcome {
    #[must_use]
    pub const fn record(&self) -> &HostDeliveryRecord {
        match self {
            Self::Enqueued(record) | Self::AlreadyQueued(record) => record,
        }
    }

    #[must_use]
    pub const fn is_new(&self) -> bool {
        matches!(self, Self::Enqueued(_))
    }

    #[must_use]
    pub fn into_record(self) -> HostDeliveryRecord {
        match self {
            Self::Enqueued(record) | Self::AlreadyQueued(record) => record,
        }
    }
}
