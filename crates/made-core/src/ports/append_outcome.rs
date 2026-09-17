use crate::entities::AuditRecord;
use crate::ports::PositionedRecord;
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

    /// The records that landed, each with its place in the global
    /// order — none for a conflict.
    ///
    /// Derived rather than stored: the positions of one append are
    /// contiguous from `first_position` by contract, so keeping one
    /// per record would be the same fact written `n` times and `n`
    /// chances for an adapter to write it differently. Whoever needs
    /// the pair — a projection being told what was just sealed, a
    /// consumer placing a cursor — gets it the same way here.
    #[must_use]
    pub fn positioned(&self) -> Vec<PositionedRecord> {
        let Self::Appended {
            records,
            first_position,
            ..
        } = self
        else {
            return Vec::new();
        };
        let mut position = *first_position;
        records
            .iter()
            .map(|record| {
                let at = position;
                position = position.next();
                PositionedRecord {
                    position: at,
                    record: record.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;
    use crate::entities::ceremony_events::CeremonyCompleted;
    use crate::entities::{AuditFact, CeremonyEvent};
    use crate::value_objects::{
        AuditActor, AuditActorKind, CeremonyId, CeremonyName, CeremonyVersion, EventId, StateId,
    };

    fn record(event_id: &str) -> AuditRecord {
        AuditRecord::first(AuditFact {
            event_id: EventId::new(event_id).unwrap(),
            event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: OffsetDateTime::UNIX_EPOCH,
            }),
            ceremony_id: CeremonyId::new("c1").unwrap(),
            definition_name: CeremonyName::new("positions").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .unwrap()
    }

    #[test]
    fn positions_of_an_append_run_on_from_the_first() {
        let outcome = AppendOutcome::Appended {
            version: StreamVersion::new(3),
            records: vec![record("e1"), record("e2"), record("e3")],
            first_position: GlobalPosition::new(7).unwrap(),
        };

        let positioned = outcome.positioned();

        assert_eq!(
            positioned
                .iter()
                .map(|entry| entry.position.value())
                .collect::<Vec<_>>(),
            [7, 8, 9]
        );
        assert_eq!(
            positioned
                .iter()
                .map(|entry| entry.record.clone())
                .collect::<Vec<_>>(),
            outcome.records()
        );
    }

    #[test]
    fn a_conflict_positions_nothing() {
        let outcome = AppendOutcome::Conflict {
            expected: StreamVersion::EMPTY,
            actual: StreamVersion::new(2),
        };

        assert!(outcome.positioned().is_empty());
        assert!(outcome.records().is_empty());
        assert!(outcome.appended_version().is_none());
        assert!(outcome.is_conflict());
    }
}
