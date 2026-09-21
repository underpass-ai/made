//! The three things a host can come back and say.

use made_core::value_objects::{
    DeliveryFailureReason, DeliveryNote, EvidenceReference, ProcessedActionRef,
};

/// The three things a host can come back and say.
///
/// Intent and effect are deliberately two calls, in that order. A host
/// records what it is about to do while it still holds the lease, does
/// it through the ordinary authorized commands, and only then says it
/// is done. A single call after the fact cannot tell a crash mid-effect
/// from an effect that never started, which is the difference between
/// resuming and doing the work twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegratorAcknowledgement {
    /// "I have it, and this is what I am about to do."
    ///
    /// Keeps the lease: the host is still holding the item.
    Intent {
        action: ProcessedActionRef,
        note: DeliveryNote,
        evidence: Option<EvidenceReference>,
    },
    /// "I did it." Closes the delivery, naming the act.
    Processed { action: ProcessedActionRef },
    /// "I could not." Counts an attempt and may offer it again.
    Failed { reason: DeliveryFailureReason },
}
