//! What a seat contributes: the schemas of the `ceremony_participation`
//! capability group.
//!
//! Split out of `ceremony_schemas.rs` unchanged. Six schemas describing
//! one thing — how a participant puts something to the table, answers
//! it, backs the answer with evidence, closes it, and says why one
//! thing here led to another — and the reference type the last of
//! those points with.

use serde_json::{json, Value};

use super::super::schema_primitives::{attributes_schema, string_schema, MAX_ID_LIST_ITEMS};
use super::intervention_delivery_schemas::{
    intervention_delivery_policy_schema, supervisor_principal_schema,
};

/// Something this session produced that a reason can point at.
fn ceremony_record_ref_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind"],
        "description": description,
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["step", "agenda_item", "contribution", "guard_decision", "transition"],
                "description": "Which of the five it names. Only the field it names is read."
            },
            "step_id": string_schema("For kind `step`."),
            "agenda_item": string_schema("For kind `agenda_item` or `contribution`."),
            "ordinal": {
                "type": "integer",
                "minimum": 0,
                "description": "For kind `contribution`, its place among the answers to its item, counting from zero. For kind `transition`, the move's place in the session, counting from one."
            },
            "guard_name": string_schema("For kind `guard_decision`.")
        }
    })
}

pub(crate) fn ceremony_reason_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "role_id", "role_kind", "from", "to", "kind", "why", "confidence"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "role_id": string_schema("Seat saying so, declared by this ceremony's definition."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills that seat. Declared by you, because only you know: a reason is a judgement, and whether a person or an agent made it is the first thing anyone weighing it wants to know."
            },
            "from": ceremony_record_ref_schema("What is being explained."),
            "to": ceremony_record_ref_schema("What explains it."),
            "kind": {
                "type": "string",
                "enum": [
                    "authorizes",
                    "chosen_because",
                    "achieved_by",
                    "follows_from",
                    "satisfies_constraint",
                    "violates_constraint",
                    "supersedes",
                    "contradicts"
                ],
                "description": "How the first came from the second. `authorizes` is the one a reviewer looks for first — not what happened, but what made it allowed to happen. `achieved_by` is the how, and it is what turns a resolved session from a precedent into a procedure. `answers` is absent: it states the shape of the session rather than anyone's judgement, and only the engine asserts it."
            },
            "why": string_schema("The reason itself, in one line. Required: an edge asserting a connection while declining to say how is a guess written down as a fact."),
            "confidence": {
                "type": "string",
                "enum": ["high", "medium", "low"],
                "description": "How sure you are. There is no fourth for `not sure enough to say` — a caller who would reach for it can decline to make the claim."
            }
        }
    })
}

pub(crate) fn request_ceremony_intervention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "role_id", "role_kind", "kind", "message"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("Optional stable intervention id. The server mints one when omitted."),
            "role_id": string_schema("Role requesting the intervention."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills that seat. Declared by you, because only you know: the journal records who asked the table for help, and an entry that cannot say whether a person or an agent asked is not worth the write."
            },
            "kind": {
                "type": "string",
                "enum": ["opinion", "investigation", "action"],
                "description": "Intent of the participant-created agenda item."
            },
            "target_role_ids": {
                "type": "array",
                "minItems": 0,
                "maxItems": MAX_ID_LIST_ITEMS,
                "uniqueItems": true,
                "items": { "type": "string", "minLength": 1 },
                "description": format!(
                    "Optional responding roles. Omit or pass [] to address the whole table; \
                     at most {MAX_ID_LIST_ITEMS}, each distinct."
                )
            },
            "message": string_schema("Participant's request in their own words."),
            "details": attributes_schema("Structured request context or evidence references."),
            "provenance": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "source_intervention_id",
                    "source_response_role_id",
                    "selected_role_id"
                ],
                "properties": {
                    "source_intervention_id": string_schema("Earlier intervention containing the selected proposal."),
                    "source_response_role_id": string_schema("Role whose response contained the selected proposal."),
                    "selected_role_id": string_schema("Role selected to handle the new intervention.")
                },
                "description": "Optional trace from a table proposal to the intervention created from it."
            },
            "target_agent_execution_id": string_schema("Put it to one live agent instead of to seats. Requires target_incarnation, and makes target_role_ids ignored."),
            "target_incarnation": string_schema("Which generation of that agent's process. Required with target_agent_execution_id: an execution without its generation names a name rather than a process, and a replacement would inherit the question."),
            "target_role_id": string_schema("Which seat that agent holds. Required with target_agent_execution_id, and refused when the definition does not let that role answer interventions."),
            "intent": {
                "type": "string",
                "enum": ["question", "feedback", "constraint", "checkpoint"],
                "description": "What the interruption is for, as against what kind of work it is. A question and a checkpoint are open until somebody answers; feedback and a constraint are told, not asked."
            },
            "delivery": intervention_delivery_policy_schema(),
            "supervisor": supervisor_principal_schema()
        }
    })
}

pub(crate) fn respond_to_ceremony_intervention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "intervention_id", "role_id", "role_kind", "message"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("Open intervention id."),
            "role_id": string_schema("Targeted role contributing this response."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills that seat. Declared by you, because only you know: a contribution weighed later as precedent reads differently depending on whether a person or an agent gave it."
            },
            "message": string_schema("Role response, opinion, or result."),
            "details": attributes_schema("Structured response context or evidence references."),
            "delivery_id": string_schema("Answer as the agent that was handed the item. All three of delivery_id, agent_execution_id and incarnation together or none: the ledger is asked whether this delivery was acknowledged by this agent before the answer is sealed."),
            "agent_execution_id": string_schema("The execution giving this answer."),
            "incarnation": string_schema("Its process generation.")
        }
    })
}

pub(crate) fn close_ceremony_intervention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "intervention_id", "role_id", "role_kind"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("Open intervention id."),
            "role_id": string_schema("Requesting role closing the intervention."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills that seat. Declared by you, because only you know: closing an item is a decision that it has been answered enough, and who made it reads differently depending on what kind of party they were."
            }
        }
    })
}

pub(crate) fn collect_ceremony_evidence_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "intervention_id", "role_id", "role_kind", "source_id", "query"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("Open investigation or action intervention receiving the evidence."),
            "role_id": string_schema("Targeted role represented by the configured evidence source."),
            "role_kind": {
                "type": "string",
                "enum": ["human", "agent", "service", "engine"],
                "description": "What kind of party fills that seat. Declared by you, because only you know: this call answers the item as well as fetching what backs the answer, and it is recorded the same way a plain response is."
            },
            "source_id": string_schema("Host-configured evidence source, such as observability."),
            "query": string_schema("Specific read-only evidence request in the participant's words."),
            "details": attributes_schema("Structured query parameters such as time window or service identity.")
        }
    })
}
