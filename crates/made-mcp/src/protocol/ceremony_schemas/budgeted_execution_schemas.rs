use serde_json::{json, Value};

use super::super::budget_schemas::{budget_limits_schema, budget_reservation_schema};
use super::super::default_idempotency_key::DEFAULT_IDEMPOTENCY_KEY_RULE;
use super::super::default_lease_owner::DEFAULT_LEASE_OWNER_RULE;
use super::super::default_lease_ttl::{
    lease_ttl_rule, CLAIM_CEREMONY_STEP_LEASE_TTL_MS, RUN_CEREMONY_LEASE_TTL_MS,
};
use super::super::schema_primitives::{attributes_schema, string_schema};

pub(in crate::protocol) fn start_published_ceremony_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony", "version", "actor_id", "actor_kind"],
        "properties": {
            "actor_id": string_schema("Who is opening it, in whatever terms you identify callers by. Not a role from the definition: at the start its roles are not filled yet, and whoever opens a session may be a participant, an operator, or a scheduler that never takes part."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party that is. Refused when missing or unrecognised, like every other actor kind."
            },
            "ceremony": string_schema("Name of the published ceremony to run."),
            "version": string_schema("Published version to bind this instance to."),
            "ceremony_id": string_schema("Identifier for the new instance. Generated when omitted."),
            "context": attributes_schema("Opening context for the working session."),
            "budget_limits": budget_limits_schema()
        }
    })
}

pub(in crate::protocol) fn run_ceremony_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["definition_yaml", "actor_id", "actor_kind"],
        "properties": {
            "ceremony_id": string_schema("Optional stable ceremony instance id. The server mints one when omitted."),
            "definition_yaml": string_schema("Declarative ceremony YAML definition."),
            "actor_id": string_schema("Who is opening it, in whatever terms you identify callers by. Not a role from the definition: at the start its roles are not filled yet, and whoever opens a session may be a participant, an operator, or a scheduler that never takes part."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party that is. Refused when missing or unrecognised, like every other actor kind."
            },
            "context": attributes_schema("Opaque initial ceremony context forwarded to guards and handlers."),
            "lease_owner_id": string_schema(DEFAULT_LEASE_OWNER_RULE),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": lease_ttl_rule(RUN_CEREMONY_LEASE_TTL_MS)
            },
            "budget_reservation": budget_reservation_schema()
        }
    })
}

pub(in crate::protocol) fn claim_ceremony_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "step_id", "actor_kind"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "step_id": string_schema("Next declared step that the host will execute outside the ceremony engine."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills the step's declared seat. The engine records this declaration and never infers it."
            },
            "lease_owner_id": string_schema(DEFAULT_LEASE_OWNER_RULE),
            "idempotency_key": string_schema(DEFAULT_IDEMPOTENCY_KEY_RULE),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": lease_ttl_rule(CLAIM_CEREMONY_STEP_LEASE_TTL_MS)
            },
            "budget_reservation": budget_reservation_schema()
            ,"execution_profile": execution_profile_schema()
        }
    })
}

fn execution_profile_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "requested_model", "requested_reasoning_effort", "required_capabilities",
            "fallback_policy", "actual_model", "actual_reasoning_effort",
            "actual_capabilities", "host_agent_id", "host_agent_incarnation"
        ],
        "properties": {
            "requested_model": string_schema("Host-requested model; not a ceremony-definition invariant."),
            "requested_reasoning_effort": string_schema("Host-requested reasoning effort."),
            "required_capabilities": {"type": "array", "items": string_schema("Required host capability.")},
            "fallback_policy": {"type": "string", "enum": ["reject", "fallback"]},
            "fallback_model": string_schema("Model to use when fallback is explicit."),
            "fallback_reasoning_effort": string_schema("Reasoning effort to use when fallback is explicit."),
            "actual_model": string_schema("Model actually selected by the host."),
            "actual_reasoning_effort": string_schema("Reasoning effort actually selected by the host."),
            "actual_capabilities": {"type": "array", "items": string_schema("Capability supplied by the host; it must be in the host inventory and include every required capability.")},
            "host_agent_id": string_schema("Host-owned agent identity; distinct from MADE role and Codex/Claude ids."),
            "host_agent_incarnation": string_schema("Host-owned incarnation identity."),
            "inherited_from": {"type": "string", "enum": ["role-default", "step-default", "ceremony-default", "host-default", "checkpoint"], "description": "Closed source category for explicit profile inheritance."},
            "checkpoint_id": string_schema("Checkpoint provenance for a resumed or handed-off step."),
            "handoff_from": string_schema("Prior host agent/incarnation provenance; does not mutate a running model.")
        }
    })
}
