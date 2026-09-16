//! Proto → JSON for what a session left behind.
//!
//! The record mapper has one job and it is exact: rebuild, key for key
//! and type for type, the serde form of `AuditRecord` that the
//! in-process arm answers with. A client reads either arm's answer
//! back into the record and verifies the chain; a key spelled
//! differently, an integer turned into a double or an absent field
//! turned into an empty string would all break that, and the parity
//! session compares the two answers field for field so they cannot.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Map, Value};

use super::primitives::pb_struct_to_json;

pub(crate) fn read_ceremony_events_to_json(response: pb::ReadCeremonyEventsResponse) -> Value {
    let pb::ReadCeremonyEventsResponse {
        records,
        next_version,
        head_version,
    } = response;
    let record_count = records.len();
    json!({
        "records": records
            .into_iter()
            .map(ceremony_event_record_to_json)
            .collect::<Vec<_>>(),
        "record_count": record_count,
        "next_version": next_version,
        "head_version": head_version,
        "has_more": next_version < head_version,
    })
}

fn ceremony_event_record_to_json(record: pb::CeremonyEventRecord) -> Value {
    let pb::CeremonyEventRecord {
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
        previous_record_hash,
        record_hash,
    } = record;
    json!({
        "event_id": event_id,
        "event_type": event_type,
        "schema_version": schema_version,
        "ceremony_id": ceremony_id,
        "definition_name": definition_name,
        "definition_version": definition_version,
        "sequence": sequence,
        "occurred_at": occurred_at,
        "actor": actor.map_or(Value::Null, ceremony_event_actor_to_json),
        "correlation_id": absent_when_empty(correlation_id),
        "causation_id": absent_when_empty(causation_id),
        "trace_id": absent_when_empty(trace_id),
        // Zero is the version-1 record that has no payload to shape.
        "event_schema_version": (event_schema_version > 0).then_some(event_schema_version),
        "event": event.map_or(Value::Null, |event| Value::Object(pb_struct_to_json(event))),
        "previous_record_hash": digest_to_json(previous_record_hash),
        // A record always carries its own digest; an empty one would
        // be a record this server could not have sealed.
        "record_hash": Value::Array(digest_bytes(record_hash)),
    })
}

fn ceremony_event_actor_to_json(actor: pb::CeremonyEventActor) -> Value {
    let mut fields = Map::new();
    fields.insert("actor_id".to_owned(), Value::String(actor.actor_id));
    fields.insert("kind".to_owned(), Value::String(actor.kind));
    fields.insert("role_id".to_owned(), absent_when_empty(actor.role_id));
    Value::Object(fields)
}

/// A digest as the record's own serde form writes it: the bytes, not a
/// rendering of them, so a client can read the record straight back.
fn digest_to_json(digest: Vec<u8>) -> Value {
    if digest.is_empty() {
        return Value::Null;
    }
    Value::Array(digest_bytes(digest))
}

fn digest_bytes(digest: Vec<u8>) -> Vec<Value> {
    digest.into_iter().map(|byte| json!(byte)).collect()
}

/// Proto3 has no absent scalar, so an empty string on the wire is the
/// `null` the in-process arm answers with.
fn absent_when_empty(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

pub(crate) fn ceremony_transcript_to_json(response: pb::GetCeremonyTranscriptResponse) -> Value {
    let entry_count = response.entries.len();
    json!({
        "entries": response
            .entries
            .into_iter()
            .map(|entry| json!({
                "step_id": entry.step_id,
                "role_id": entry.role_id,
                "output": entry
                    .output
                    .map_or_else(|| Value::Object(Map::new()), |output| {
                        Value::Object(pb_struct_to_json(output))
                    }),
            }))
            .collect::<Vec<_>>(),
        "entry_count": entry_count,
    })
}

pub(crate) fn ceremony_report_to_json(response: pb::GenerateCeremonyReportResponse) -> Value {
    json!({
        "report_markdown": response.report_markdown,
        "ceremony_ids": response.ceremony_ids,
        "ceremony_count": response.ceremony_count,
        "completed_count": response.completed_count,
        "incomplete_count": response.incomplete_count,
        "definition_bindings": response
            .definition_bindings
            .into_iter()
            .map(|binding| json!({
                "ceremony_id": binding.ceremony_id,
                "definition_name": binding.definition_name,
                "definition_version": binding.definition_version,
                "definition_digest": binding.definition_digest,
                "bound_definition_digest": absent_when_empty(binding.bound_definition_digest),
            }))
            .collect::<Vec<_>>(),
        // Not a field of the response: no edition writes a report
        // anywhere, and an always-false boolean on the wire would
        // suggest a caller could ask for one that is.
        "persisted": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> pb::CeremonyEventRecord {
        pb::CeremonyEventRecord {
            event_id: "e1".to_owned(),
            event_type: "ceremony_instance_started".to_owned(),
            schema_version: 2,
            ceremony_id: "session-1".to_owned(),
            definition_name: "planning_ceremony".to_owned(),
            definition_version: "1.0".to_owned(),
            sequence: 1,
            occurred_at: "2026-09-16T09:00:00Z".to_owned(),
            actor: Some(pb::CeremonyEventActor {
                actor_id: "operator-1".to_owned(),
                kind: "service".to_owned(),
                role_id: String::new(),
            }),
            correlation_id: "e1".to_owned(),
            causation_id: String::new(),
            trace_id: String::new(),
            event_schema_version: 1,
            event: None,
            previous_record_hash: Vec::new(),
            record_hash: vec![0_u8; 32],
        }
    }

    #[test]
    fn an_absent_optional_comes_back_null_and_not_as_an_empty_string() {
        let json = ceremony_event_record_to_json(record());

        assert_eq!(json["correlation_id"], json!("e1"));
        assert_eq!(json["causation_id"], Value::Null);
        assert_eq!(json["trace_id"], Value::Null);
        assert_eq!(json["actor"]["role_id"], Value::Null);
        assert_eq!(json["previous_record_hash"], Value::Null);
        assert_eq!(json["event"], Value::Null);
    }

    #[test]
    fn a_digest_travels_as_the_bytes_the_record_holds() {
        let json = ceremony_event_record_to_json(record());

        let bytes = json["record_hash"].as_array().expect("a digest is bytes");
        assert_eq!(bytes.len(), 32);
        assert!(bytes.iter().all(Value::is_u64));
    }

    /// The keys are the record's own, because a client reads them back
    /// into the record. Spelled here so a rename fails loudly.
    #[test]
    fn a_record_carries_exactly_the_keys_the_stored_record_has() {
        let json = ceremony_event_record_to_json(record());

        let mut keys: Vec<&str> = json
            .as_object()
            .expect("a record is an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "actor",
                "causation_id",
                "ceremony_id",
                "correlation_id",
                "definition_name",
                "definition_version",
                "event",
                "event_id",
                "event_schema_version",
                "event_type",
                "occurred_at",
                "previous_record_hash",
                "record_hash",
                "schema_version",
                "sequence",
                "trace_id",
            ]
        );
    }

    #[test]
    fn a_page_says_whether_there_is_more() {
        let caught_up = read_ceremony_events_to_json(pb::ReadCeremonyEventsResponse {
            records: vec![record()],
            next_version: 3,
            head_version: 3,
        });
        assert_eq!(caught_up["record_count"], json!(1));
        assert_eq!(caught_up["has_more"], json!(false));

        let behind = read_ceremony_events_to_json(pb::ReadCeremonyEventsResponse {
            records: Vec::new(),
            next_version: 1,
            head_version: 3,
        });
        assert_eq!(behind["record_count"], json!(0));
        assert_eq!(behind["has_more"], json!(true));
    }

    #[test]
    fn a_transcript_entry_without_output_answers_with_an_empty_object() {
        let json = ceremony_transcript_to_json(pb::GetCeremonyTranscriptResponse {
            entries: vec![pb::CeremonyTranscriptEntry {
                step_id: "draft".to_owned(),
                role_id: "WRITER".to_owned(),
                output: None,
            }],
        });

        assert_eq!(json["entry_count"], json!(1));
        assert_eq!(json["entries"][0]["output"], json!({}));
    }

    #[test]
    fn a_session_bound_to_nothing_reports_a_null_bound_digest() {
        let json = ceremony_report_to_json(pb::GenerateCeremonyReportResponse {
            report_markdown: "# Ceremony report".to_owned(),
            ceremony_ids: vec!["session-1".to_owned()],
            ceremony_count: 1,
            completed_count: 0,
            incomplete_count: 1,
            definition_bindings: vec![pb::CeremonyReportBinding {
                ceremony_id: "session-1".to_owned(),
                definition_name: "planning_ceremony".to_owned(),
                definition_version: "1.0".to_owned(),
                definition_digest: "abc".to_owned(),
                bound_definition_digest: String::new(),
            }],
        });

        assert_eq!(
            json["definition_bindings"][0]["bound_definition_digest"],
            Value::Null
        );
        assert_eq!(json["persisted"], json!(false));
    }
}
