use crate::value_objects::HostDeliveryRecord;

/// What writing down an activation did to a delivery.
///
/// An activation is a push, so the answers are not the leased ones.
/// Reaching a host is transport and nothing more: a delivered record
/// stays offerable, and the host still has to come and take the work
/// under its own lease. What an operator needs to tell apart afterwards
/// is a deployment that does not wake hosts from one that tried and
/// could not, which is why `NotAttempted` is an answer rather than a
/// silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedActivation {
    /// The host was reached, and the receipt says how.
    Delivered(HostDeliveryRecord),
    /// This deployment does not wake hosts; nothing was written.
    NotAttempted(HostDeliveryRecord),
    /// The wake-up failed and the delivery has attempts left.
    Requeued(HostDeliveryRecord),
    /// The wake-up failed and the delivery ran out of attempts.
    Exhausted(HostDeliveryRecord),
    /// The delivery already ended; an activation does not reopen it.
    AlreadyEnded(HostDeliveryRecord),
    /// No delivery of that identity is held.
    Unknown,
}

impl RecordedActivation {
    #[must_use]
    pub const fn record(&self) -> Option<&HostDeliveryRecord> {
        match self {
            Self::Delivered(record)
            | Self::NotAttempted(record)
            | Self::Requeued(record)
            | Self::Exhausted(record)
            | Self::AlreadyEnded(record) => Some(record),
            Self::Unknown => None,
        }
    }

    /// Whether the host is known to have been reached.
    #[must_use]
    pub const fn reached_the_host(&self) -> bool {
        matches!(self, Self::Delivered(_))
    }
}
