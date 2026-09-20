use crate::protocol::schema_primitives::{attributes_schema, string_schema, tool_def};
use crate::protocol::tool_names::{PLAN_CEREMONY_SUCCESSOR_TOOL, START_CEREMONY_SUCCESSOR_TOOL};
use serde_json::{json, Value};

fn disposition_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["step_id", "claim_fence", "kind"],
            "properties": {
                "step_id": string_schema("Step holding the outstanding claim."),
                "claim_fence": string_schema("The exact claim the plan observed. A stale fence fails the plan."),
                "kind": {"type": "string", "enum": [
                    "abandon_no_external_effect",
                    "abandon_effect_reconciled",
                    "carry_receipt",
                    "retry_in_successor"
                ]},
                "evidence": string_schema("Where the reconciled effect was established. Required with abandon_effect_reconciled."),
                "receipt_id": string_schema("Receipt the successor answers for. Required with carry_receipt.")
            }
        }
    })
}

pub(super) fn plan_tool() -> Value {
    tool_def(
        PLAN_CEREMONY_SUCCESSOR_TOOL,
        "Read what handing this ceremony to a published successor would involve: the definition diff, the whole resume preflight, the evidence a successor could start from, the disposition every outstanding claim would require, the changes that would strand work already completed here, and what is in the way. Seals nothing and starts nothing.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["ceremony_id", "definition_name", "definition_version"],
            "properties": {
                "ceremony_id": string_schema("The paused ceremony that would hand off."),
                "definition_name": string_schema("Published definition the successor would run."),
                "definition_version": string_schema("Published version of that definition.")
            }
        }),
    )
}

pub(super) fn start_tool() -> Value {
    tool_def(
        START_CEREMONY_SUCCESSOR_TOOL,
        "Seal the handoff in the paused ceremony and open the successor it names. The predecessor is sealed first, so a retry after a crash verifies the successor's opening instead of making a second one. The predecessor can no longer resume; it can still be cancelled. The successor seals its own deadlines and starts with a fresh budget.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["ceremony_id", "plan_id", "definition_name", "definition_version", "actor_id", "actor_kind"],
            "properties": {
                "ceremony_id": string_schema("The paused ceremony handing off."),
                "plan_id": string_schema("Caller's identity for this handoff. The successor's id derives from it, so retrying with it lands once; reusing it for different content conflicts."),
                "definition_name": string_schema("Published definition the successor runs."),
                "definition_version": string_schema("Published version of that definition."),
                "carried": {
                    "type": "array",
                    "items": string_schema("Step of the successor that starts from this ceremony's sealed work.")
                },
                "dispositions": disposition_schema(),
                "budget": {"type": "string", "enum": ["fresh"], "default": "fresh"},
                "context_overrides": attributes_schema("Context for the successor. Omit to carry this ceremony's context unchanged."),
                "actor_id": string_schema("Who is deciding the handoff."),
                "actor_kind": {"type": "string", "enum": ["human", "agent", "service"]}
            }
        }),
    )
}
