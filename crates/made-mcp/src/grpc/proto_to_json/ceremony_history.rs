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
use crate::protocol::{CeremonyJournalVerdictView, REPORT_IS_PERSISTED};
use crate::renderers::{AuditRecordView, CeremonyEventPageView};

pub(crate) fn read_ceremony_events_to_json(response: pb::ReadCeremonyEventsResponse) -> Value {
    let pb::ReadCeremonyEventsResponse {
        records,
        next_version,
        head_version,
    } = response;
    let records = records
        .into_iter()
        .map(ceremony_event_record_view)
        .collect();
    CeremonyEventPageView::new(records, next_version, head_version).to_json()
}

/// The verdict on one journal's chain.
///
/// Through the same view the in-process arm fills, so the two answers
/// are one shape by construction. Zero and empty are how the contract
/// says "absent"; they become `null` here, which is how this server
/// says it everywhere else.
pub(crate) fn verify_ceremony_journal_to_json(
    response: pb::VerifyCeremonyJournalResponse,
) -> Value {
    let pb::VerifyCeremonyJournalResponse {
        ceremony_id,
        head_version,
        record_count,
        intact,
        first_broken_sequence,
        reason,
    } = response;
    CeremonyJournalVerdictView {
        ceremony_id,
        head_version,
        record_count: u64::from(record_count),
        intact,
        first_broken_sequence: (first_broken_sequence > 0).then_some(first_broken_sequence),
        reason: (!reason.is_empty()).then_some(reason),
    }
    .to_json()
}

fn ceremony_event_record_view(record: pb::CeremonyEventRecord) -> AuditRecordView {
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
    AuditRecordView {
        event_id,
        event_type,
        schema_version,
        ceremony_id,
        definition_name,
        definition_version,
        sequence,
        occurred_at,
        actor: actor.map_or(Value::Null, ceremony_event_actor_to_json),
        correlation_id: empty_to_option(correlation_id),
        causation_id: empty_to_option(causation_id),
        trace_id: empty_to_option(trace_id),
        // Zero is the version-1 record that has no payload to shape.
        event_schema_version: (event_schema_version > 0).then_some(event_schema_version),
        event: event.map(|event| Value::Object(pb_struct_to_json(event))),
        previous_record_hash: (!previous_record_hash.is_empty()).then_some(previous_record_hash),
        record_hash,
    }
}

fn ceremony_event_actor_to_json(actor: pb::CeremonyEventActor) -> Value {
    let mut fields = Map::new();
    fields.insert("actor_id".to_owned(), Value::String(actor.actor_id));
    fields.insert("kind".to_owned(), Value::String(actor.kind));
    fields.insert(
        "role_id".to_owned(),
        empty_to_option(actor.role_id).map_or(Value::Null, Value::String),
    );
    Value::Object(fields)
}

fn empty_to_option(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
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
                "bound_definition_digest": empty_to_option(binding.bound_definition_digest),
            }))
            .collect::<Vec<_>>(),
        // Not a field of the response; the constant says why.
        "persisted": REPORT_IS_PERSISTED,
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
        let json = ceremony_event_record_view(record()).to_json();

        assert_eq!(json["correlation_id"], json!("e1"));
        assert_eq!(json["causation_id"], Value::Null);
        assert_eq!(json["trace_id"], Value::Null);
        assert_eq!(json["actor"]["role_id"], Value::Null);
        assert_eq!(json["previous_record_hash"], Value::Null);
        assert_eq!(json["event"], Value::Null);
    }

    #[test]
    fn a_digest_travels_as_the_bytes_the_record_holds() {
        let json = ceremony_event_record_view(record()).to_json();

        let bytes = json["record_hash"].as_array().expect("a digest is bytes");
        assert_eq!(bytes.len(), 32);
        assert!(bytes.iter().all(Value::is_u64));
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
