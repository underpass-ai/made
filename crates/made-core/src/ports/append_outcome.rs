use crate::entities::AuditRecord;
use crate::value_objects::{GlobalPosition, StreamVersion};

/// What an event store did with a batch of facts.
///
/// A conflict is an outcome rather than an error: another caller got
/// there first, nothing landed, and the right response is to reload
/// and decide again — not to give up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendOutcome {
    /// Every fact was sealed and landed. `version` is the stream's new
    /// head, `records` the sealed facts in order, and `first_position`
    /// where the first of them sits in the global order; the rest
    /// follow it contiguously.
    Appended {
        version: StreamVersion,
        records: Vec<AuditRecord>,
        first_position: GlobalPosition,
    },
    /// The stream was not at the version the caller decided against.
    /// Nothing was written.
    Conflict {
        expected: StreamVersion,
        actual: StreamVersion,
    },
}

impl AppendOutcome {
    #[must_use]
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict { .. })
    }

    /// The records that landed — none for a conflict.
    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        match self {
            Self::Appended { records, .. } => records,
            Self::Conflict { .. } => &[],
        }
    }

    /// The stream's head after the append, if it landed.
    #[must_use]
    pub fn appended_version(&self) -> Option<StreamVersion> {
        match self {
            Self::Appended { version, .. } => Some(*version),
            Self::Conflict { .. } => None,
        }
    }
}
