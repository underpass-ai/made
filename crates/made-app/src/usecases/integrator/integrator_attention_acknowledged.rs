//! What the ledger did with an acknowledgement.

use made_core::ports::{AckOutcome, DeliveryFailureOutcome, ProcessedOutcome};
use made_core::value_objects::ProcessedActionRef;

/// The answer to one acknowledgement, shaped like what was said.
///
/// The three arms are not interchangeable and a caller has to handle
/// them apart: an intent that was refused leaves the host holding
/// something it must not act on, and a `Processed` that conflicts
/// means somebody else already closed the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegratorAttentionAcknowledged {
    /// The intent is on the record and the lease is still held.
    Intent {
        outcome: AckOutcome,
        /// What the host said it was about to do, echoed back so a
        /// caller reading a transcript can see intent and effect as one
        /// pair rather than two unrelated calls.
        action: Box<ProcessedActionRef>,
    },
    /// The delivery is closed, or was already.
    Processed(ProcessedOutcome),
    /// The attempt was counted.
    Failed(DeliveryFailureOutcome),
}
