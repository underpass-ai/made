use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::entities::CeremonyInstance;
use crate::value_objects::{
    AuditRecordHash, CeremonyId, CeremonyName, CeremonyRevision, CeremonyVersion,
};

/// A session written before ceremonies were streams was brought into
/// one (ADR-012).
///
/// The genesis event, and the only one that opens a stream without the
/// session having been started in it. A store from v0.3.x holds a
/// snapshot and a journal of receipts with no payloads; the events that
/// produced that snapshot cannot be recovered, so the import states
/// what it can prove — the state as the legacy row held it, the head of
/// the journal that stands beside it, and when the import happened.
///
/// It carries the aggregate itself rather than a copy of its fields.
/// The claim the import makes is that folding the new stream yields the
/// snapshot that was imported, and a payload assembled from a
/// hand-written field list would make that claim only as complete as
/// the list: a field added to the session later would quietly stop
/// being imported. This way it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceImported {
    pub ceremony_id: CeremonyId,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
    /// The session exactly as the legacy store held it.
    ///
    /// Boxed because it is by far the largest thing any event carries,
    /// and an enum is as big as its widest variant: inline, one import
    /// would have made every `StepCompleted` in every stream pay for
    /// it. `Box` is transparent to serde, so the stored shape is the
    /// session's own.
    pub snapshot: Box<CeremonyInstance>,
    /// The digest of the last record of the legacy journal, when it had
    /// one. It is provenance, not a link: the new stream's first record
    /// chains to nothing, because the legacy records seal no payloads
    /// and cannot be continued.
    pub legacy_journal_head_hash: Option<AuditRecordHash>,
    /// The revision the legacy row was at when it was read.
    pub legacy_revision: CeremonyRevision,
    #[serde(with = "time::serde::rfc3339")]
    pub imported_at: OffsetDateTime,
}
