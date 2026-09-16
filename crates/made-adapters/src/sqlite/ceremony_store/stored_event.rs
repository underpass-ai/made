use made_core::entities::AuditRecord;
use made_core::value_objects::GlobalPosition;
use serde::{Deserialize, Serialize};

/// A sealed record and the global position the store filed it at.
///
/// The position travels with the record rather than only in the log
/// so a stream read never has to consult the log to say where each of
/// its records sits among every other stream's.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredEvent {
    pub(super) position: GlobalPosition,
    pub(super) record: AuditRecord,
}
