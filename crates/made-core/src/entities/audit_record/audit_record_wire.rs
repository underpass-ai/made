use serde::Deserialize;
use time::OffsetDateTime;

use crate::entities::CeremonyEventReader;
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditEventType, AuditRecordHash, AuditSequence, CeremonyId, CeremonyName,
    CeremonyVersion, EventId, EventSchemaVersion,
};

use super::AuditRecord;

/// The stored shape of a record, before its payload has been read.
#[derive(Deserialize)]
pub(super) struct AuditRecordWire {
    event_id: EventId,
    event_type: AuditEventType,
    schema_version: u32,
    ceremony_id: CeremonyId,
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    sequence: AuditSequence,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
    actor: AuditActor,
    #[serde(default)]
    correlation_id: Option<EventId>,
    #[serde(default)]
    causation_id: Option<EventId>,
    #[serde(default)]
    trace_id: Option<String>,
    #[serde(default)]
    event_schema_version: Option<EventSchemaVersion>,
    #[serde(default)]
    event: Option<serde_json::Value>,
    #[serde(default)]
    previous_record_hash: Option<AuditRecordHash>,
    record_hash: AuditRecordHash,
}

impl TryFrom<AuditRecordWire> for AuditRecord {
    type Error = DomainError;

    fn try_from(wire: AuditRecordWire) -> Result<Self, Self::Error> {
        let event = match (wire.event, wire.event_schema_version) {
            (Some(raw), Some(version)) => {
                Some(CeremonyEventReader::read(wire.event_type, version, raw)?)
            }
            (Some(_), None) => {
                return Err(DomainError::InvariantViolated {
                    reason:
                        "an audit record carrying an event must name its payload schema version",
                });
            }
            (None, _) => None,
        };
        Ok(Self {
            event_id: wire.event_id,
            event_type: wire.event_type,
            schema_version: wire.schema_version,
            ceremony_id: wire.ceremony_id,
            definition_name: wire.definition_name,
            definition_version: wire.definition_version,
            sequence: wire.sequence,
            occurred_at: wire.occurred_at,
            actor: wire.actor,
            correlation_id: wire.correlation_id,
            causation_id: wire.causation_id,
            trace_id: wire.trace_id,
            event_schema_version: wire.event_schema_version,
            event,
            previous_record_hash: wire.previous_record_hash,
            record_hash: wire.record_hash,
        })
    }
}
