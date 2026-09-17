use crate::entities::CeremonyInstance;
use crate::value_objects::{AuditRecordHash, CeremonyRevision};

/// One session a pre-stream store holds, with everything the import
/// can state about it.
///
/// The definition binding travels inside the session, where it has
/// always lived: an instance started from a published definition
/// carries its digest, and one started from a document carries none.
/// Splitting it out here would have invited an importer to decide the
/// binding for itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCeremonySnapshot {
    pub instance: CeremonyInstance,
    pub revision: CeremonyRevision,
    /// The digest of the last record of the legacy journal, when the
    /// session has one. A session with no journal is still importable:
    /// what cannot be stated is left absent rather than invented.
    pub journal_head_hash: Option<AuditRecordHash>,
}
