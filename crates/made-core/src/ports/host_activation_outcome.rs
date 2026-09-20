use crate::value_objects::{DeliveryFailureReason, HostActivationReceipt};

/// What an activation adapter did with an envelope.
///
/// `Unsupported` is a first-class answer rather than an error: a
/// deployment with no adapter is a normal deployment, and its hosts
/// find their work by asking for it. Telling that apart from a wake-up
/// that failed is what lets an operator know whether to go looking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostActivationOutcome {
    /// The host was reached. Transport, not processing.
    Accepted(HostActivationReceipt),
    /// This deployment does not wake hosts.
    Unsupported,
    /// The adapter tried and could not.
    Failed(DeliveryFailureReason),
}

impl HostActivationOutcome {
    #[must_use]
    pub const fn receipt(&self) -> Option<&HostActivationReceipt> {
        match self {
            Self::Accepted(receipt) => Some(receipt),
            Self::Unsupported | Self::Failed(_) => None,
        }
    }

    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }
}
