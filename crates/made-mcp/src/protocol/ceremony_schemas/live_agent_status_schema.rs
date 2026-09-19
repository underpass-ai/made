use serde_json::{json, Value};

use super::super::schema_primitives::string_schema;

pub(in crate::protocol) fn list_ceremony_agents_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Ceremony whose bounded live-agent roster is read."),
            "execution_status": {"type": "string", "enum": ["running", "waiting", "blocked", "finished"]},
            "cursor": string_schema("Opaque bounded-page cursor."),
            "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 50}
        }
    })
}

pub(in crate::protocol) fn get_ceremony_agent_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["ceremony_id", "agent_execution_id"],
        "properties": {
            "ceremony_id": string_schema("Ceremony containing the execution."),
            "agent_execution_id": string_schema("Logical execution identity, not a host agent id.")
        }
    })
}

pub(in crate::protocol) fn report_ceremony_agent_status_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["status"],
        "properties": {
            "status": {
                "type": "object", "additionalProperties": false,
                "required": ["ceremony_id", "agent_execution_id", "operation_id", "claim_owner_id", "logical_worker_id", "host_agent_id", "host_agent_incarnation", "role_id", "step_id", "execution_status", "liveness", "source", "activity", "task_summary", "observed_at", "report_sequence", "idempotency_key", "claim_fence"],
                "properties": {
                    "ceremony_id": string_schema("Authorized ceremony."),
                    "agent_execution_id": string_schema("Stable logical execution identity."),
                    "operation_id": {"type": "string", "pattern": "^[0-9a-f]{64}$", "description": "Durable external operation identity, distinct from the logical execution."},
                    "claim_owner_id": string_schema("Current accepted lease owner and authenticated reporting principal."),
                    "logical_worker_id": string_schema("Logical worker identity across host replacement."),
                    "host_agent_id": string_schema("Opaque host executor identity."),
                    "host_agent_incarnation": string_schema("Opaque host incarnation; changes on replacement."),
                    "previous_host_agent_id": string_schema("Previous executor during handoff."),
                    "previous_host_agent_incarnation": string_schema("Previous incarnation during handoff."),
                    "role_id": string_schema("MADE role being executed."), "step_id": string_schema("MADE step."),
                    "attempt": {"type": "integer", "minimum": 0},
                    "execution_status": {"type": "string", "enum": ["running", "waiting", "blocked", "finished"]},
                    "liveness": {"type": "string", "enum": ["fresh", "stale", "unreachable", "unknown"]},
                    "source": {"type": "string", "enum": ["host_report", "derived_lease", "host_discovery_unsupported"]},
                    "requested_model": string_schema("Requested host model, if known."), "requested_reasoning_effort": string_schema("Requested effort, if known."),
                    "actual_model": string_schema("Actual host model, if known."), "actual_reasoning_effort": string_schema("Actual effort, if known."),
                    "activity": string_schema("Bounded public activity label; checkpoint_available, intervention_delivered, intervention_answered, input_requested, failed and heartbeat receive dedicated stream kinds. No chain-of-thought."), "blocker": string_schema("Bounded blocker label; adding or clearing it produces blocker_opened or blocker_resolved."), "dependency": string_schema("Bounded dependency identity."), "task_summary": string_schema("Bounded task summary; no credentials or private chain-of-thought."),
                    "evidence_references": {"type": "array", "maxItems": 20, "items": string_schema("Evidence reference, never secret content.")},
                    "usage_kind": string_schema("measured, estimated or unavailable."), "usage_value": {"type": "integer", "minimum": 0},
                    "observed_at": string_schema("RFC3339 observation timestamp."), "report_sequence": {"type": "integer", "minimum": 1}, "idempotency_key": string_schema("Stable report idempotency key."), "claim_fence": string_schema("Accepted MADE claim fence.")
                }
            }
        }
    })
}
