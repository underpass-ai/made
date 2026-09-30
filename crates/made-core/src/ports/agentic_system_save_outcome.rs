use crate::value_objects::AgenticSystemRevision;

/// What happened when a design was saved against the revision its
/// author had read.
///
/// A conflict carries the revision that is actually stored, because
/// "somebody else edited this" is not enough to act on: the editor has
/// to be able to fetch what they missed and re-apply their change.
///
/// An edit of a design that was never stored is neither of those. It
/// is not a conflict — there is no revision to name, and naming one
/// would send the editor to read something that does not exist and
/// retry forever against it — so it has its own answer: the id is
/// wrong, or the design has to be created first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticSystemSaveOutcome {
    Saved {
        revision: AgenticSystemRevision,
    },
    RevisionConflict {
        current: AgenticSystemRevision,
    },
    /// An expected revision was given for a design with no revisions
    /// at all. Nothing was written.
    Absent,
}

impl AgenticSystemSaveOutcome {
    #[must_use]
    pub const fn saved(revision: AgenticSystemRevision) -> Self {
        Self::Saved { revision }
    }

    #[must_use]
    pub const fn conflict(current: AgenticSystemRevision) -> Self {
        Self::RevisionConflict { current }
    }

    #[must_use]
    pub const fn absent() -> Self {
        Self::Absent
    }

    #[must_use]
    pub const fn revision(self) -> Option<AgenticSystemRevision> {
        match self {
            Self::Saved { revision } => Some(revision),
            Self::RevisionConflict { .. } | Self::Absent => None,
        }
    }

    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self, Self::RevisionConflict { .. })
    }

    #[must_use]
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }
}
