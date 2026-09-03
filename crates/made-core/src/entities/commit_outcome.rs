use crate::entities::AuditRecord;
use crate::value_objects::{CeremonyRevision, ExpectedRevision};

/// Result of atomically committing ceremony state, audit records and messages.
#[derive(Debug, Clone, PartialEq)]
pub enum CommitOutcome {
    Committed {
        revision: CeremonyRevision,
        records: Vec<AuditRecord>,
    },
    Conflict {
        expected: ExpectedRevision,
        stored: Option<CeremonyRevision>,
    },
}

impl CommitOutcome {
    #[must_use]
    pub fn committed_revision(&self) -> Option<CeremonyRevision> {
        match self {
            Self::Committed { revision, .. } => Some(*revision),
            Self::Conflict { .. } => None,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        match self {
            Self::Committed { records, .. } => records,
            Self::Conflict { .. } => &[],
        }
    }

    #[must_use]
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict { .. })
    }
}
