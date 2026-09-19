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
        }
    })
}
