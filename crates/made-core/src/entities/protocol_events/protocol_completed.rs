use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::StateId;

/// The protocol reached a terminal state.
///
/// Sealed in the same commit as the transition that got it there. A
/// fold stamps `completed_at` from here rather than deciding for itself
/// which states are terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompleted {
    pub final_state: StateId,
    #[serde(with = "time::serde::rfc3339")]
    pub completed_at: OffsetDateTime,
}
