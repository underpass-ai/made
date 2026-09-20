use crate::value_objects::{
    HostDeliveryRecord, HostDeliveryStateKind, IntegratorFence, ProcessedActionRef,
};

/// What the ledger did with a host's claim to have acted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessedOutcome {
    /// Closed by this act.
    Processed(HostDeliveryRecord),
    /// Already closed by the same act; nothing changed.
    AlreadyProcessed(HostDeliveryRecord),
    /// Already closed by a different act.
    Conflict { existing: Box<ProcessedActionRef> },
    /// Nothing was acknowledged yet, so there is nothing to close.
    ///
    /// A host that claims to have acted on something it never said it
    /// received has lost track of which delivery it is answering, and
    /// accepting the claim would close a hand-off nobody made.
    NotAcknowledged { state: HostDeliveryStateKind },
    /// The caller's generation is not the one currently bound.
    FenceRejected { current: IntegratorFence },
    /// The delivery belongs to a different host generation.
    LeaseNotOwned,
}

impl ProcessedOutcome {
    #[must_use]
    pub const fn record(&self) -> Option<&HostDeliveryRecord> {
        match self {
            Self::Processed(record) | Self::AlreadyProcessed(record) => Some(record),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Processed(_) | Self::AlreadyProcessed(_))
    }
}
