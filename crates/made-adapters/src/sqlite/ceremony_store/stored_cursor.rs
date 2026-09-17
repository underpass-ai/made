use made_core::value_objects::{
    CeremonyEventCursorAttempt, CeremonyEventCursorLease, GlobalPosition,
};
use serde::{Deserialize, Serialize};

/// Durable state of one named consumer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct StoredCursor {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: CeremonyEventCursorAttempt,
    pub(super) lease: Option<CeremonyEventCursorLease>,
}
