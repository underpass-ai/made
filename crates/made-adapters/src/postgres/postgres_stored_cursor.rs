use made_core::value_objects::{
    GlobalPosition, ProtocolEventCursorAttempt, ProtocolEventCursorLease,
};
use serde::{Deserialize, Serialize};

/// Serialized state of one Postgres-backed global-feed consumer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct PostgresStoredCursor {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: ProtocolEventCursorAttempt,
    pub(super) lease: Option<ProtocolEventCursorLease>,
}
