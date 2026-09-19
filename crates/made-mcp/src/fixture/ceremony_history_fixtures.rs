//! Canned answers for the reads of what a session left behind.
//!
//! The record's shape is the one thing worth getting exactly right
//! here: it is the serde form of a stored `AuditRecord`, digest bytes
//! and all, because a client wiring itself against the fixture backend
//! should be reading the same JSON a real engine sends.

use serde_json::{json, Value};

use crate::renderers::{AuditRecordView, CeremonyEventPageView};

/// The first record of the fixture session, sealed and whole.
pub(super) fn read_ceremony_events_fixture() -> Value {
    let record = AuditRecordView {
        global_position: None,
        authorization: None,
        event_id: "ceremony-fixture-1:started".to_owned(),
        event_type: "ceremony_instance_started".to_owned(),
        schema_version: 2,
        ceremony_id: "ceremony-fixture-1".to_owned(),
        definition_name: "fixture_ceremony".to_owned(),
        definition_version: "1.0".to_owned(),
        sequence: 1,
        occurred_at: "2026-01-01T00:00:00Z".to_owned(),
        actor: json!({
            "actor_id": "operator-fixture-1",
            "kind": "service",
            "role_id": null
        }),
        correlation_id: Some("ceremony-fixture-1:started".to_owned()),
        causation_id: None,
        trace_id: None,
        event_schema_version: Some(1),
        event: Some(json!({
            "type": "ceremony_instance_started",
            "ceremony_id": "ceremony-fixture-1",
            "definition_name": "fixture_ceremony",
            "definition_version": "1.0",
            "initial_state": "DECIDE",
            "step_ids": ["decide"],
            "context": { "brief": "ship the editorial calendar" },
            "bound_definition": null,
            "created_at": "2026-01-01T00:00:00Z"
        })),
        previous_record_hash: None,
        record_hash: FIXTURE_DIGEST.to_vec(),
    };
    CeremonyEventPageView::new(vec![record], 1, 1).to_json()
}

pub(super) fn pull_ceremony_events_fixture() -> Value {
    let mut page = read_ceremony_events_fixture();
    let mut record = page["records"][0].take();
    record["global_position"] = json!(1);
    json!({ "records": [record], "acknowledged_through": null })
}

pub(super) fn stream_ceremony_fixture() -> Value {
    let page = read_ceremony_events_fixture();
    json!({
        "records": page["records"].clone(),
        "resume_after_sequence": 1,
        "head_sequence": 1,
        "end_reason": "event_limit",
    })
}

/// The one-record fixture stream, whole: a fixture that answered
/// "broken" would teach a client to expect a store that is.
pub(super) fn verify_ceremony_journal_fixture() -> Value {
    json!({
        "ceremony_id": "ceremony-fixture-1",
        "head_version": 1,
        "record_count": 1,
        "intact": true,
        "first_broken_sequence": null,
        "reason": null,
    })
}

pub(super) fn ceremony_transcript_fixture() -> Value {
    json!({
        "entries": [
            {
                "step_id": "decide",
                "role_id": "DECIDER",
                "output": { "decision": "ship the smaller scope first" }
            }
        ],
        "entry_count": 1
    })
}

pub(super) fn ceremony_report_fixture() -> Value {
    json!({
        "report_markdown": FIXTURE_REPORT_MARKDOWN,
        "ceremony_ids": ["ceremony-fixture-1"],
        "ceremony_count": 1,
        "completed_count": 1,
        "incomplete_count": 0,
        "definition_bindings": [
            {
                "ceremony_id": "ceremony-fixture-1",
                "definition_name": "fixture_ceremony",
                "definition_version": "1.0",
                "definition_digest": "0000000000000000000000000000000000000000000000000000000000000000",
                "bound_definition_digest": null
            }
        ],
        "persisted": false
    })
}

/// Thirty-two bytes, as a stored digest is. Zeroes rather than a
/// plausible hash: a fixture that looked like a real digest would
/// invite somebody to verify it.
const FIXTURE_DIGEST: [u8; 32] = [0; 32];

const FIXTURE_REPORT_MARKDOWN: &str = "# Ceremony report

Ceremonies: 1 · completed: 1 · incomplete: 0

## Ceremony `ceremony-fixture-1`

- Definition: `fixture_ceremony`
- Version: `1.0`
- State: `COMPLETED`
- Status: `completed`
";
