use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::CeremonyId;

use super::{MemoryEntry, MemoryEntryId, MemoryEntryKind};

/// One thing an earlier session left behind, as a later one reads it.
///
/// Not a [`MemoryEntry`]. An entry is what was written, with the
/// evidence behind it and whatever detail the writer attached; this is
/// what a session is handed at its opening, and it carries only what a
/// reader can act on without going anywhere else — what it says, what
/// kind of thing it is, and which session said it when.
///
/// The session and the moment travel with it because a decision nobody
/// can attribute is not a decision a later session can weigh. The
/// evidence does not: it is a reference into somewhere else, and a
/// recollection that carried references the reader has no way to follow
/// would be offering a door into a wall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecalledEntry {
    id: MemoryEntryId,
    kind: MemoryEntryKind,
    summary: String,
    /// The session that said it.
    from_ceremony: CeremonyId,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl RecalledEntry {
    /// Infallible: everything here was already validated when the entry
    /// was written, and re-checking it would let a recollection fail
    /// over a memory somebody else wrote.
    #[must_use]
    pub fn of(entry: &MemoryEntry) -> Self {
        Self {
            id: entry.id().clone(),
            kind: entry.kind(),
            summary: entry.summary().to_owned(),
            from_ceremony: entry.provenance().ceremony_id().clone(),
            observed_at: entry.provenance().observed_at(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &MemoryEntryId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> MemoryEntryKind {
        self.kind
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn from_ceremony(&self) -> &CeremonyId {
        &self.from_ceremony
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }

    /// How much of the budget this entry costs.
    pub(super) fn cost(&self) -> usize {
        self.summary.len()
    }
}
