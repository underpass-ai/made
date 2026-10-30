use made_core::value_objects::{
    GlobalPosition, ProtocolEventCursorAttempt, ProtocolEventCursorLease,
};
use serde::{Deserialize, Serialize};

/// Durable state of one named consumer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct StoredCursor {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: ProtocolEventCursorAttempt,
    pub(super) lease: Option<ProtocolEventCursorLease>,
}
