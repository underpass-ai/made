use serde::Deserialize;
use time::OffsetDateTime;

use crate::entities::CeremonyEventReader;
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditEventType, AuditRecordHash, AuditSequence, AuthorizationEvidence, CeremonyId,
    CeremonyName, CeremonyVersion, EventId, EventSchemaVersion,
};

use super::{
    AuditRecord, AUTHORIZED_SCHEMA_VERSION, EVENT_BEARING_SCHEMA_VERSION, LEGACY_SCHEMA_VERSION,
};

/// Stored record shapes, selected before any fields are admitted to the domain.
///
/// V1 and v2 remain flat. V3 is deliberately enveloped so a v0.6 reader,
/// which expects the flat required fields, rejects it before it can append.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum AuditRecordWire {
    Flat {
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
    },
    Authorized {
        schema_version: u32,
        record: serde_json::Value,
        authorization: AuthorizationEvidence,
    },
}

impl TryFrom<AuditRecordWire> for AuditRecord {
    type Error = DomainError;

    fn try_from(wire: AuditRecordWire) -> Result<Self, Self::Error> {
        match wire {
            flat @ AuditRecordWire::Flat { .. } => AuditRecord::from_flat_wire(flat),
            authorized @ AuditRecordWire::Authorized { .. } => {
                AuditRecord::from_authorized_wire(authorized)
            }
        }
    }
}

impl AuditRecord {
    fn from_flat_wire(wire: AuditRecordWire) -> Result<Self, DomainError> {
        let AuditRecordWire::Flat {
            event_id,
            event_type,
            schema_version,
            ceremony_id,
            definition_name,
            definition_version,
            sequence,
            occurred_at,
            actor,
            correlation_id,
            causation_id,
            trace_id,
            event_schema_version,
            event: raw_event,
            previous_record_hash,
            record_hash,
        } = wire
        else {
            unreachable!("flat wire selected by variant")
        };
        if !matches!(
            schema_version,
            LEGACY_SCHEMA_VERSION | EVENT_BEARING_SCHEMA_VERSION
        ) {
            return Err(unsupported_schema());
        }
        let event = match (raw_event, event_schema_version) {
            (Some(raw), Some(version)) => {
                Some(CeremonyEventReader::read(event_type, version, raw)?)
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
            event_id,
            event_type,
            schema_version,
            ceremony_id,
            definition_name,
            definition_version,
            sequence,
            occurred_at,
            actor,
            correlation_id,
            causation_id,
            trace_id,
            event_schema_version,
            event,
            authorization_evidence: None,
            previous_record_hash,
            record_hash,
        })
    }

    fn from_authorized_wire(wire: AuditRecordWire) -> Result<Self, DomainError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct AuthorizedRecordBody {
            event_id: EventId,
            event_type: AuditEventType,
            ceremony_id: CeremonyId,
            definition_name: CeremonyName,
            definition_version: CeremonyVersion,
            sequence: AuditSequence,
            #[serde(with = "time::serde::rfc3339")]
            occurred_at: OffsetDateTime,
            actor: AuditActor,
            correlation_id: Option<EventId>,
            causation_id: Option<EventId>,
            trace_id: Option<String>,
            event_schema_version: EventSchemaVersion,
            event: serde_json::Value,
            previous_record_hash: Option<AuditRecordHash>,
            record_hash: AuditRecordHash,
        }

        let AuditRecordWire::Authorized {
            schema_version,
            record,
            authorization,
        } = wire
        else {
            unreachable!("authorized wire selected by variant")
        };
        if schema_version != AUTHORIZED_SCHEMA_VERSION {
            return Err(unsupported_schema());
        }
        let body: AuthorizedRecordBody =
            serde_json::from_value(record).map_err(|_| DomainError::InvariantViolated {
                reason: "version-3 audit record body is malformed",
            })?;
        Self::validate_authorization_evidence(&authorization, body.occurred_at)?;
        let event =
            CeremonyEventReader::read(body.event_type, body.event_schema_version, body.event)?;
        Ok(Self {
            event_id: body.event_id,
            event_type: body.event_type,
            schema_version,
            ceremony_id: body.ceremony_id,
            definition_name: body.definition_name,
            definition_version: body.definition_version,
            sequence: body.sequence,
            occurred_at: body.occurred_at,
            actor: body.actor,
            correlation_id: body.correlation_id,
            causation_id: body.causation_id,
            trace_id: body.trace_id,
            event_schema_version: Some(body.event_schema_version),
            event: Some(event),
            authorization_evidence: Some(authorization),
            previous_record_hash: body.previous_record_hash,
            record_hash: body.record_hash,
        })
    }
}

fn unsupported_schema() -> DomainError {
    DomainError::InvariantViolated {
        reason: "audit record schema version is unsupported",
    }
}
