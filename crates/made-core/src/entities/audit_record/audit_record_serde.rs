use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::OffsetDateTime;

use crate::entities::CeremonyEvent;
use crate::value_objects::{
    AuditActor, AuditEventType, AuditRecordHash, AuditSequence, AuthorizationEvidence, CeremonyId,
    CeremonyName, CeremonyVersion, EventId, EventSchemaVersion,
};

use super::audit_record_wire::AuditRecordWire;
use super::{
    AuditRecord, AUTHORIZED_SCHEMA_VERSION, EVENT_BEARING_SCHEMA_VERSION, LEGACY_SCHEMA_VERSION,
};

impl Serialize for AuditRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.schema_version {
            LEGACY_SCHEMA_VERSION | EVENT_BEARING_SCHEMA_VERSION => {
                serialize_flat(self, serializer)
            }
            AUTHORIZED_SCHEMA_VERSION => serialize_authorized(self, serializer),
            _ => Err(<S::Error as serde::ser::Error>::custom(
                "audit record schema version is unsupported",
            )),
        }
    }
}

fn serialize_flat<S>(record: &AuditRecord, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct FlatRecord<'a> {
        event_id: &'a EventId,
        event_type: AuditEventType,
        schema_version: u32,
        ceremony_id: &'a CeremonyId,
        definition_name: &'a CeremonyName,
        definition_version: &'a CeremonyVersion,
        sequence: AuditSequence,
        #[serde(with = "time::serde::rfc3339")]
        occurred_at: OffsetDateTime,
        actor: &'a AuditActor,
        correlation_id: Option<&'a EventId>,
        causation_id: Option<&'a EventId>,
        trace_id: Option<&'a str>,
        event_schema_version: Option<EventSchemaVersion>,
        event: Option<&'a CeremonyEvent>,
        previous_record_hash: Option<AuditRecordHash>,
        record_hash: AuditRecordHash,
    }

    FlatRecord {
        event_id: &record.event_id,
        event_type: record.event_type,
        schema_version: record.schema_version,
        ceremony_id: &record.ceremony_id,
        definition_name: &record.definition_name,
        definition_version: &record.definition_version,
        sequence: record.sequence,
        occurred_at: record.occurred_at,
        actor: &record.actor,
        correlation_id: record.correlation_id.as_ref(),
        causation_id: record.causation_id.as_ref(),
        trace_id: record.trace_id.as_deref(),
        event_schema_version: record.event_schema_version,
        event: record.event.as_ref(),
        previous_record_hash: record.previous_record_hash,
        record_hash: record.record_hash,
    }
    .serialize(serializer)
}

fn serialize_authorized<S>(record: &AuditRecord, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct AuthorizedRecordBody<'a> {
        event_id: &'a EventId,
        event_type: AuditEventType,
        ceremony_id: &'a CeremonyId,
        definition_name: &'a CeremonyName,
        definition_version: &'a CeremonyVersion,
        sequence: AuditSequence,
        #[serde(with = "time::serde::rfc3339")]
        occurred_at: OffsetDateTime,
        actor: &'a AuditActor,
        correlation_id: Option<&'a EventId>,
        causation_id: Option<&'a EventId>,
        trace_id: Option<&'a str>,
        event_schema_version: EventSchemaVersion,
        event: &'a CeremonyEvent,
        previous_record_hash: Option<AuditRecordHash>,
        record_hash: AuditRecordHash,
    }

    #[derive(Serialize)]
    struct AuthorizedEnvelope<'a> {
        schema_version: u32,
        record: AuthorizedRecordBody<'a>,
        authorization: &'a AuthorizationEvidence,
    }

    let (Some(event_schema_version), Some(event), Some(authorization)) = (
        record.event_schema_version,
        record.event.as_ref(),
        record.authorization_evidence.as_ref(),
    ) else {
        return Err(<S::Error as serde::ser::Error>::custom(
            "version-3 audit record is incomplete",
        ));
    };
    AuthorizedEnvelope {
        schema_version: AUTHORIZED_SCHEMA_VERSION,
        record: AuthorizedRecordBody {
            event_id: &record.event_id,
            event_type: record.event_type,
            ceremony_id: &record.ceremony_id,
            definition_name: &record.definition_name,
            definition_version: &record.definition_version,
            sequence: record.sequence,
            occurred_at: record.occurred_at,
            actor: &record.actor,
            correlation_id: record.correlation_id.as_ref(),
            causation_id: record.causation_id.as_ref(),
            trace_id: record.trace_id.as_deref(),
            event_schema_version,
            event,
            previous_record_hash: record.previous_record_hash,
            record_hash: record.record_hash,
        },
        authorization,
    }
    .serialize(serializer)
}

impl<'de> Deserialize<'de> for AuditRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        AuditRecordWire::deserialize(deserializer)?
            .try_into()
            .map_err(<D::Error as serde::de::Error>::custom)
    }
}
