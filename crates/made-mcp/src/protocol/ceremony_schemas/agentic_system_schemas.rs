//! Request schemas for the agentic-system tools.
//!
//! The design document is declared as an object whose shape the
//! application owns, not mirrored key by key here. Two descriptions of
//! one document are two places for them to disagree, and the decoder
//! already refuses unknown fields — which is the check that matters,
//! because a misspelled key silently ignored is a supervision policy
//! that quietly is not there.

use serde_json::{json, Value};

use crate::protocol::schema_primitives::string_schema;

/// The document `made_design_agentic_system` takes.
pub(crate) fn agentic_system_design_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["design"],
        "properties": {
            "design": {
                "type": "object",
                "description": "The design document: id, purpose, integrator_role_id, roles, participants, topology, profiles, ceremonies (each with a pin of name, version and optional digest), supervision and attention. expected_revision is the revision you read; omit it only when creating. Unknown fields are refused.",
                "required": ["id", "purpose", "integrator_role_id", "roles", "ceremonies"],
                "properties": {
                    "id": string_schema("Stable identity of the design."),
                    "expected_revision": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "The revision you read. Omit to create."
                    },
                    "purpose": {"type": "string", "minLength": 1, "maxLength": 2000},
                    "integrator_role_id": string_schema("Which declared role drives the system."),
                    "roles": {"type": "array", "minItems": 1},
                    "participants": {"type": "array"},
                    "topology": {"type": "array"},
                    "profiles": {"type": "object"},
                    "ceremonies": {"type": "array", "minItems": 1},
                    "supervision": {"type": "object"},
                    "attention": {"type": "object"}
                }
            }
        }
    })
}

/// A design named by identity, optionally at a revision.
pub(crate) fn agentic_system_read_schema(subject: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["system_id"],
        "properties": {
            "system_id": string_schema(subject),
            "revision": {
                "type": "integer",
                "minimum": 1,
                "description": "Omitted reads the head."
            }
        }
    })
}

pub(crate) fn agentic_system_list_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "lifecycle": {
                "type": "string",
                "enum": ["draft", "published", "deprecated"],
                "description": "Omitted lists every lifecycle."
            },
            "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 50},
            "after": string_schema("Last identifier of the previous page.")
        }
    })
}

pub(crate) fn agentic_system_publish_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["system_id", "revision"],
        "properties": {
            "system_id": string_schema("Design to seal."),
            "revision": {
                "type": "integer",
                "minimum": 1,
                "description": "The revision you read and want sealed. Required: sealing whatever happens to be the head would seal something nobody checked."
            }
        }
    })
}

pub(crate) fn agentic_system_instantiate_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["system_id", "revision", "execution_id", "actor_id", "actor_kind"],
        "properties": {
            "system_id": string_schema("Design to run."),
            "revision": {"type": "integer", "minimum": 1},
            "execution_id": string_schema(
                "Your own name for this run, and its idempotency key. Asking twice answers with the run that exists."
            ),
            "inputs": {
                "type": "object",
                "description": "Starting context per composed ceremony, keyed by the name it goes by inside the system."
            },
            "offers": {
                "type": "object",
                "description": "What your host can supply per logical participant: {specialty, capabilities[]}. A participant you do not offer is recorded unavailable and the ceremonies needing it are skipped with the reason.",
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["specialty"],
                    "properties": {
                        "specialty": string_schema("Which seat label the host will fill."),
                        "capabilities": {"type": "array", "items": {"type": "string"}}
                    }
                }
            },
            "integrator_destination": {
                "type": "object",
                "additionalProperties": false,
                "required": ["host_kind", "address"],
                "properties": {
                    "host_kind": string_schema("What sort of host this is."),
                    "address": string_schema("Where to reach it, in your own terms."),
                    "activation_mode": {"type": "string", "enum": ["none", "command"]}
                }
            },
            "actor_id": string_schema("Who is opening the run, in your own terms."),
            "actor_kind": {"type": "string", "enum": ["human", "agent", "service", "engine"]}
        }
    })
}

/// Reading a run takes nothing but its identity.
pub(crate) fn agentic_system_execution_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["execution_id"],
        "properties": {
            "execution_id": string_schema("Run to read.")
        }
    })
}

/// Advancing one is an act, and an act is attributed.
///
/// Its own schema rather than the read one with optional fields:
/// starting a ceremony writes a record of who started it, and an
/// actor the caller could omit would be an actor the engine had to
/// invent.
pub(crate) fn agentic_system_advance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["execution_id", "actor_id", "actor_kind"],
        "properties": {
            "execution_id": string_schema("Run to advance."),
            "actor_id": string_schema("Who is advancing it, in your own terms."),
            "actor_kind": {"type": "string", "enum": ["human", "agent", "service", "engine"]}
        }
    })
}

pub(crate) fn agentic_system_diagram_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["system_id"],
        "properties": {
            "system_id": string_schema("Design to draw."),
            "revision": {"type": "integer", "minimum": 1},
            "execution_id": string_schema(
                "Draw this run's pinned design with its observed progress. Omitted draws the design alone."
            )
        }
    })
}
