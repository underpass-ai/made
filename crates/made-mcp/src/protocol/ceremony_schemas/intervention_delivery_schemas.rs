//! Getting an intervention in front of the agent that is working, and
//! knowing whether it arrived.
//!
//! Four schemas, and the shared pieces the extended participation
//! schemas next door also use. The recipient is spelled out the same
//! way everywhere — execution, incarnation, role — because all three
//! are needed to say "this agent and not the one that replaced it",
//! and a shape that allowed two of the three would let a caller
//! address a name rather than a process.

use serde_json::{json, Value};

use super::super::schema_primitives::string_schema;

/// The terms an item is offered to a host under.
pub(crate) fn intervention_delivery_policy_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "How this item is offered to hosts. Every field is optional; omitted ones take the engine's defaults.",
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["pull_lease", "activation"],
                "description": "Whether the host asks for its work or the engine wakes it. Defaults to pull_lease."
            },
            "lease_duration_ms": {
                "type": "integer",
                "minimum": 1,
                "maximum": 3_600_000,
                "description": "How long a host may hold this exclusively. Bounded at both ends: a lease of no length excludes nobody, and an unbounded one strands the item behind a host that never comes back."
            },
            "ack_timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "description": "How long an acknowledged offer may sit before it expires. Omitted means it never does."
            },
            "max_attempts": {
                "type": "integer",
                "minimum": 1,
                "maximum": 16,
                "description": "How many times this may be offered before the failure stays visible. Defaults to 3."
            },
            "follow_replacement": {
                "type": "boolean",
                "description": "Whether the question follows an agent that gets replaced, or stays with the process that was asked and dies with it. Defaults to false."
            }
        }
    })
}

/// Somebody who may ask without holding a seat.
pub(crate) fn supervisor_principal_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["principal_id", "display"],
        "description": "Ask on behalf of somebody outside the table. The item is recorded as asked by a role derived from this principal, which no definition can declare, so asking buys nothing but the asking. Requires ambient authorization naming the same principal.",
        "properties": {
            "principal_id": string_schema("The authenticated principal asking."),
            "display": string_schema("How that principal is named to the agent it interrupts.")
        }
    })
}

pub(crate) fn pull_ceremony_agent_interventions_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "agent_execution_id", "incarnation", "role_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "agent_execution_id": string_schema("The live execution asking for its questions."),
            "incarnation": string_schema("Which generation of the process is asking. A replacement reusing the execution id is a different agent and is not handed its predecessor's questions."),
            "role_id": string_schema("The seat that execution holds."),
            "lease_duration_ms": {
                "type": "integer",
                "minimum": 1,
                "maximum": 3_600_000,
                "description": "How long to hold what is handed over. Defaults to 60000. A lease and not a take: an agent that dies holding one strands nothing."
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 100,
                "description": "How many to take at once. Defaults to 100."
            }
        }
    })
}

pub(crate) fn acknowledge_ceremony_agent_intervention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "ceremony_id",
            "intervention_id",
            "delivery_id",
            "lease_id",
            "agent_execution_id",
            "incarnation",
            "role_id",
            "observation_kind"
        ],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("The item being acknowledged."),
            "delivery_id": string_schema("The offer being acknowledged, as the pull handed it over."),
            "lease_id": string_schema("The lease from the same pull. A foreign or expired one is refused with no effect."),
            "agent_execution_id": string_schema("The execution acknowledging."),
            "incarnation": string_schema("Its process generation."),
            "role_id": string_schema("The seat it holds."),
            "observation_kind": {
                "type": "string",
                "enum": ["received", "refused", "incapable", "busy", "timeout"],
                "description": "What you saw. `received` and `refused` and `incapable` are statements about the item and are sealed in the ceremony's stream; `busy` and `timeout` are statements about you, count an attempt and put the offer back for whoever can take it."
            },
            "note": string_schema("What to record alongside it, in your own words."),
            "evidence": string_schema("An evidence reference backing what you say you saw."),
            "observed_at": string_schema("When you saw it, RFC3339. State it if you might retry: an acknowledgement that differs only by the instant it was resent conflicts with its own retry. Omitted means now.")
        }
    })
}

pub(crate) fn get_ceremony_intervention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "intervention_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "intervention_id": string_schema("The item to read, with every route it took.")
        }
    })
}

pub(crate) fn list_ceremony_interventions_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id"],
        "properties": {
            "ceremony_id": string_schema("Started ceremony instance id."),
            "status": {
                "type": "string",
                "enum": [
                    "recorded",
                    "queued",
                    "delivered",
                    "acknowledged",
                    "responded",
                    "closed",
                    "failed",
                    "expired",
                    "unsupported"
                ],
                "description": "Only items whose projected delivery status is this. Computed from the stream and the ledger together; `delivered` always has a lease or an activation receipt behind it."
            },
            "role_id": string_schema("Only items a given seat may answer."),
            "agent_execution_id": string_schema("Only items addressed to, or routed to, a given execution."),
            "unresolved_only": {
                "type": "boolean",
                "description": "Only items somebody still owes something. Defaults to false."
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 100,
                "description": "Page size. Defaults to 100."
            },
            "cursor": string_schema("The next_cursor of a previous page.")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledging_names_the_ticket_as_well_as_the_item() {
        let schema = acknowledge_ceremony_agent_intervention_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        // Without the lease, an acknowledgement is an unverifiable
        // claim by whoever sent it.
        assert!(required.contains(&"lease_id"));
        assert!(required.contains(&"delivery_id"));
        assert!(required.contains(&"incarnation"));
        assert_eq!(schema["additionalProperties"], json!(false));
        // Stating the instant is what makes a retry repeatable.
        assert!(schema["properties"]["observed_at"].is_object());
    }

    #[test]
    fn pulling_names_the_process_and_not_only_the_execution() {
        let schema = pull_ceremony_agent_interventions_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert!(required.contains(&"incarnation"));
        assert!(required.contains(&"role_id"));
    }

    #[test]
    fn listing_offers_only_statuses_the_projection_can_produce() {
        let schema = list_ceremony_interventions_schema();
        let statuses = schema["properties"]["status"]["enum"].as_array().unwrap();
        assert_eq!(statuses.len(), 9);
        assert!(statuses.contains(&json!("delivered")));
        assert!(statuses.contains(&json!("recorded")));
    }
}
