use made_core::value_objects::{
    CeremonyEventCursorAttempt, CeremonyEventCursorLease, GlobalPosition,
};
use serde::{Deserialize, Serialize};

/// Serialized state of one Postgres-backed global-feed consumer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct PostgresStoredCursor {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: CeremonyEventCursorAttempt,
    pub(super) lease: Option<CeremonyEventCursorLease>,
}
