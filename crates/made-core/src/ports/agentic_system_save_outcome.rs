use crate::value_objects::AgenticSystemRevision;

/// What happened when a design was saved against the revision its
/// author had read.
///
/// A conflict carries the revision that is actually stored, because
/// "somebody else edited this" is not enough to act on: the editor has
/// to be able to fetch what they missed and re-apply their change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticSystemSaveOutcome {
    Saved { revision: AgenticSystemRevision },
    RevisionConflict { current: AgenticSystemRevision },
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
    pub const fn revision(self) -> Option<AgenticSystemRevision> {
        match self {
            Self::Saved { revision } => Some(revision),
            Self::RevisionConflict { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self, Self::RevisionConflict { .. })
    }
}
