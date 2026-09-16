use crate::entities::AuditRecord;
use crate::value_objects::GlobalPosition;

/// A sealed record together with its place in the order every stream
/// shares.
///
/// The position is not part of the record: the digest covers what
/// happened in one ceremony, and where the store filed it among other
/// ceremonies is the store's business. Pairing them here is what a
/// cross-stream reader needs to keep a cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedRecord {
    pub position: GlobalPosition,
    pub record: AuditRecord,
}
