//! Canned answers for the four reads of what a session left behind.
//!
//! The record's shape is the one thing worth getting exactly right
//! here: it is the serde form of a stored `AuditRecord`, digest bytes
//! and all, because a client wiring itself against the fixture backend
//! should be reading the same JSON a real engine sends.

use serde_json::{json, Value};

/// The first record of the fixture session, sealed and whole.
pub(super) fn read_ceremony_events_fixture() -> Value {
    json!({
        "records": [
            {
                "event_id": "ceremony-fixture-1:started",
                "event_type": "ceremony_instance_started",
                "schema_version": 2,
                "ceremony_id": "ceremony-fixture-1",
                "definition_name": "fixture_ceremony",
                "definition_version": "1.0",
                "sequence": 1,
                "occurred_at": "2026-01-01T00:00:00Z",
                "actor": {
                    "actor_id": "operator-fixture-1",
                    "kind": "service",
                    "role_id": null
                },
                "correlation_id": "ceremony-fixture-1:started",
                "causation_id": null,
                "trace_id": null,
                "event_schema_version": 1,
                "event": {
                    "type": "ceremony_instance_started",
                    "ceremony_id": "ceremony-fixture-1",
                    "definition_name": "fixture_ceremony",
                    "definition_version": "1.0",
                    "initial_state": "DECIDE",
                    "step_ids": ["decide"],
                    "context": { "brief": "ship the editorial calendar" },
                    "bound_definition": null,
                    "created_at": "2026-01-01T00:00:00Z"
                },
                "previous_record_hash": null,
                "record_hash": FIXTURE_DIGEST
            }
        ],
        "record_count": 1,
        "next_version": 1,
        "head_version": 1,
        "has_more": false
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
