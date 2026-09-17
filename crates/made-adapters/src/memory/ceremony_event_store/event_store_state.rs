use std::collections::BTreeMap;

use made_core::entities::{AuditRecord, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::PositionedRecord;
use made_core::value_objects::{AuditSequence, CeremonyId, GlobalPosition, StreamVersion};

type LogEntry = (GlobalPosition, CeremonyId, AuditSequence);

#[derive(Debug, Default)]
pub(super) struct EventStoreState {
    pub(super) streams: BTreeMap<CeremonyId, Vec<AuditRecord>>,
    pub(super) log: Vec<LogEntry>,
    pub(super) snapshots: BTreeMap<CeremonyId, BTreeMap<StreamVersion, CeremonyInstance>>,
}

impl EventStoreState {
    pub(super) fn stream(&self, stream: &CeremonyId) -> &[AuditRecord] {
        self.streams.get(stream).map_or(&[], Vec::as_slice)
    }

    pub(super) fn next_position(&self) -> GlobalPosition {
        self.log
            .last()
            .map_or(GlobalPosition::FIRST, |(position, _, _)| position.next())
    }

    pub(super) fn record_at(&self, entry: &LogEntry) -> Result<PositionedRecord, DomainError> {
        let (position, stream, sequence) = entry;
        let index = usize::try_from(sequence.value().saturating_sub(1)).ok();
        let record = index
            .and_then(|index| self.stream(stream).get(index))
            .ok_or(DomainError::InvariantViolated {
                reason: "in-memory event store: the global log points at a missing record",
            })?;
        Ok(PositionedRecord {
            position: *position,
            record: record.clone(),
        })
    }
}
