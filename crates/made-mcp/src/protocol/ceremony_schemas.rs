use serde_json::{json, Value};

use super::default_idempotency_key::DEFAULT_IDEMPOTENCY_KEY_RULE;
use super::default_lease_owner::DEFAULT_LEASE_OWNER_RULE;
use super::default_lease_ttl::{
    lease_ttl_rule, CLAIM_CEREMONY_STEP_LEASE_TTL_MS, RUN_CEREMONY_LEASE_TTL_MS,
    RUN_CEREMONY_STEP_LEASE_TTL_MS,
};
use super::schema_primitives::{attributes_schema, string_schema, MAX_ID_LIST_ITEMS};
use super::struct_numbers::STRUCT_NUMBER_RULE;

mod ceremony_history_schemas;
mod ceremony_participation_schemas;

#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use ceremony_history_schemas::REPORT_IS_PERSISTED;
pub(super) use ceremony_history_schemas::{
    ceremony_report_schema, get_ceremony_transcript_schema, pull_ceremony_events_schema,
    read_ceremony_events_schema, stream_ceremony_schema, verify_ceremony_journal_schema,
};
pub(super) use ceremony_participation_schemas::{
    ceremony_reason_schema, close_ceremony_intervention_schema, collect_ceremony_evidence_schema,
    request_ceremony_intervention_schema, respond_to_ceremony_intervention_schema,
};

pub(super) fn start_published_ceremony_schema() -> Value {
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
            "context": attributes_schema("Opening context for the working session.")
        }
    })
}

/// Either a published version, named, or a document supplied for the
/// occasion. Both at once has no sensible reading, and the schema says
/// so — in `oneOf`, not only in prose, so a caller's own validator
/// refuses what this server refuses.
pub(super) fn ceremony_definition_ref_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": format!(
            "{description} Exactly one of the two ways to name a definition: \
             `ceremony` with `version`, or `definition_yaml` on its own."
        ),
        "properties": {
            "ceremony": string_schema("Name of a published definition. Give this with `version`."),
            "version": string_schema("Version of a published definition. Give this with `ceremony`."),
            "definition_yaml": string_schema("A definition supplied for the comparison, instead of naming a published one.")
        },
        "oneOf": [
            {
                "required": ["ceremony", "version"],
                "not": { "required": ["definition_yaml"] }
            },
            {
                "required": ["definition_yaml"],
                "not": {
                    "anyOf": [{ "required": ["ceremony"] }, { "required": ["version"] }]
                }
            }
        ]
    })
}

pub(super) fn ceremony_draft_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["definition_yaml"],
        "properties": {
            "definition_yaml": string_schema(
                "Ceremony definition YAML to analyse. It does not need to be publishable — reporting why it is not is the point."
            )
        }
    })
}

mod design;

pub(super) use design::ceremony_design_schema;

pub(super) fn run_ceremony_schema() -> Value {
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
            }
        }
    })
}

pub(super) fn start_ceremony_schema() -> Value {
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
            "context": attributes_schema("Opaque initial ceremony context forwarded to guards and handlers.")
        }
    })
}

pub(super) fn run_ceremony_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "step_id", "actor_kind"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "step_id": string_schema("Step declared in the instance's current state."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party is running it. Declared by you, because only you know: which seat runs this step comes from the definition, and that says which seat was required, not what turned up. This records who ran the step, not what produced its output — the handler is named by a host-defined string the engine will not classify."
            },
            "lease_owner_id": string_schema(DEFAULT_LEASE_OWNER_RULE),
            "idempotency_key": string_schema(DEFAULT_IDEMPOTENCY_KEY_RULE),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": lease_ttl_rule(RUN_CEREMONY_STEP_LEASE_TTL_MS)
            }
        }
    })
}

pub(super) fn prepare_ceremony_children_schema() -> Value {
    run_ceremony_step_schema()
}

pub(super) fn accept_child_completion_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["child_id", "terminal_event_id"],
        "properties": {
            "child_id": string_schema("Child ceremony whose terminal record is being presented."),
            "terminal_event_id": string_schema("Exact CeremonyCompleted event id in the verified intact child journal. Later accepted facts may follow it.")
        }
    })
}

pub(super) fn recover_ceremony_children_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "limit": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum durable cursor records to inspect. Zero or omission uses the engine default."
            }
        }
    })
}

pub(super) fn claim_ceremony_step_schema() -> Value {
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
            }
        }
    })
}

pub(super) fn complete_ceremony_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "step_id", "actor_kind", "status", "claim_fence"],
        // What `status` is decides whether `error` belongs, and the
        // schema says so rather than only describing it: a failure with
        // no reason is not a report, and a reason attached to a success
        // is a contradiction the engine would have to resolve for the
        // caller.
        "if": {
            "required": ["status"],
            "properties": { "status": { "enum": ["failed"] } }
        },
        "then": { "required": ["error"] },
        "else": { "not": { "required": ["error"] } },
        "properties": {
            "claim_fence": {"type": "string", "pattern": "^[0-9a-f]{64}$", "description": "Identity returned by the accepted claim; retain across execution and retries."},
            "ceremony_id": string_schema("Started ceremony instance id."),
            "step_id": string_schema("Previously claimed ceremony step receiving the host's result."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party completed the work. The step's seat comes from the immutable definition."
            },
            "status": {
                "type": "string",
                "enum": ["completed", "failed", "waiting_for_human", "cancelled"],
                "description": "Observable result. `failed` requires `error`; every other status forbids it."
            },
            "output": attributes_schema("Structured host output, including evidence and artifact references. Omitted output is empty."),
            "error": string_schema("Required only for failed results and forbidden otherwise.")
        }
    })
}

pub(super) fn ceremony_guard_approval_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "guard_name", "role_id", "role_kind"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "guard_name": string_schema("Currently-blocking human guard explicitly approved by the human participant."),
            "role_id": string_schema("Seat approving it, declared by this ceremony's definition. Required: an approval that names no one is a receipt for a human decision nobody can be shown to have taken."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party filled that seat. Declared by you, because only you know: that this guard demands a human approval says one was required, not that one turned up, and an engine reading compliance off its own requirement would write exactly the receipt it refuses to write."
            }
        }
    })
}

pub(super) fn ceremony_guard_deferral_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "guard_name", "role_id", "role_kind", "statement", "reason", "reconsider_when"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "guard_name": string_schema("Currently-blocking human guard whose decision is deferred."),
            "role_id": string_schema("Seat deferring it, declared by this ceremony's definition. The fourth of what, why, when and who — and the only one nobody can reconstruct afterwards."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party filled that seat. Declared, never deduced."
            },
            "statement": string_schema("Human participant's own statement, preserved verbatim."),
            "reason": string_schema("Why the participant cannot decide yet."),
            "reconsider_when": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_ID_LIST_ITEMS,
                "uniqueItems": true,
                "items": { "type": "string", "minLength": 1 },
                "description": format!(
                    "Concrete conditions that would make it appropriate to ask again. \
                     At least one, at most {MAX_ID_LIST_ITEMS}, each distinct."
                )
            }
        }
    })
}

pub(super) fn ceremony_transition_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "trigger", "actor_kind"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "trigger": string_schema("Transition trigger declared from the instance's current state."),
            "actor_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party is firing it. Declared by you, because only you know: which seat may fire this trigger comes from the definition, and that says which seat was required, not what turned up to fill it."
            }
        }
    })
}

pub(super) fn ceremony_instance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id.")
        }
    })
}
