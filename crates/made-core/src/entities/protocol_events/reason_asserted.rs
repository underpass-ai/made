use serde::{Deserialize, Serialize};

use crate::value_objects::ProtocolReason;

/// A seat said why one thing in this protocol led to another.
///
/// The reason exactly as the aggregate keeps it: both ends, kind, the
/// why, confidence, who asserted it and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonAsserted {
    pub reason: ProtocolReason,
}
