//! The five schemas of the integrator loop.
//!
//! One file because they are one conversation: the scope named when a
//! host is bound is the scope it asks under, the fence raised by the
//! bind is the fence checked by the ask, and the lease handed out by
//! the ask is the lease the acknowledgement presents. Reading them
//! apart hides that.

use serde_json::{json, Value};

use super::super::schema_primitives::string_schema;

/// The longest a host may hold the line, whatever it asks for.
const MAX_WAIT_MS: u64 = 30_000;

/// What a host gets when it names no wait at all.
const DEFAULT_WAIT_MS: u64 = 1_000;

/// What every item in this family carries as the thing to revalidate
/// against before acting. Said in each schema rather than once in the
/// docs, because a caller reads the schema of the call it is making.
const REVALIDATE: &str = "Whatever travels with an item was true when the batch was built. \
     Read the ceremony again with made_get_ceremony_instance before acting on it.";

/// One ceremony, or one run of a composed system.
fn integrator_scope_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind"],
        "description": "What this binding covers. Two ids rather than one, because the same string can name both a ceremony and a system run, and a scope that could not tell them apart would hand one host's work to the other.",
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["ceremony", "system_execution"],
                "description": "Which of the two the scope is."
            },
            "ceremony_id": string_schema("Started ceremony instance id. Give this with kind `ceremony`."),
            "system_execution_id": string_schema("Agentic system execution id. Give this with kind `system_execution`.")
        }
    })
}

pub(crate) fn bind_ceremony_integrator_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["binding_id", "scope", "role_id", "host_kind", "address", "incarnation"],
        "description": "Put one host in charge of one scope. The binding is what everything else in this family is checked against.",
        "properties": {
            "binding_id": string_schema("The binding to create, named by the caller so a retry is the same bind."),
            "scope": integrator_scope_schema(),
            "role_id": string_schema("The seat this host plays while it drives the scope."),
            "host_kind": string_schema("What kind of host this is, as the deployment names it."),
            "address": string_schema("Where that host is reached."),
            "activation": {
                "type": "string",
                "enum": ["none", "command"],
                "description": "Whether the engine may wake this host or it comes and asks. Defaults to none. A deployment that declares `command` and has no command adapter composed still records every offer; nothing is invented."
            },
            "incarnation": string_schema("Which generation of the host process is binding. A host that restarts is a different incarnation of the same seat, and saying so is what lets the fence tell the new process from the old one."),
            "replace": {
                "type": "boolean",
                "description": "Whether a live binding for this scope may be displaced. Defaults to false, which refuses rather than taking over: a host that does not know it was replaced is the one this protects."
            },
            "follow_replacement": {
                "type": "boolean",
                "description": "Whether the displaced host's unanswered work follows the new destination, or stays behind with the reason it stopped. Defaults to false."
            }
        }
    })
}

pub(crate) fn get_ceremony_integrator_binding_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["scope"],
        "description": "Who is driving a scope now, and under which fence. Answers null when nobody is.",
        "properties": { "scope": integrator_scope_schema() }
    })
}

pub(crate) fn await_integrator_attention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["scope", "binding_id", "incarnation", "fence"],
        "description": format!(
            "Be handed whatever this integrator is owed, holding the line for a bounded while if there is nothing yet. {REVALIDATE}"
        ),
        "properties": {
            "scope": integrator_scope_schema(),
            "binding_id": string_schema("The binding asking."),
            "incarnation": string_schema("Which process is asking."),
            "fence": {
                "type": "integer",
                "minimum": 0,
                "description": "Which generation of the binding this process belongs to, as the bind answered with. A host that was replaced asking for work is refused here, which is what the fence is for."
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 100,
                "description": "How many items to take at once. Defaults to 100."
            },
            "wait_timeout_ms": {
                "type": "integer",
                "minimum": 0,
                "maximum": MAX_WAIT_MS,
                "description": format!(
                    "How long to hold the line when there is nothing yet. Defaults to {DEFAULT_WAIT_MS} and is capped at {MAX_WAIT_MS}: a bounded wait is one a caller can retry, and an unbounded one is a hung host. Ask again while `end_reason` is `wait_elapsed`; stop when `loop_state` is completed, failed, blocked or awaiting_human_decision."
                )
            },
            "lease_duration_ms": {
                "type": "integer",
                "minimum": 1,
                "maximum": 3_600_000,
                "description": "How long the items handed over are held exclusively. Defaults to the engine's own. A lease and not a take: a host that dies holding one strands nothing."
            }
        }
    })
}

pub(crate) fn acknowledge_integrator_attention_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "binding_id",
            "incarnation",
            "fence",
            "delivery_id",
            "lease_id",
            "acknowledgement"
        ],
        "description": "Say what you are about to do about one item, or what you did. Intent and effect are two calls in that order: record the intent while you still hold the lease, do the work through the ordinary authorized commands, then say it is done. A single call after the fact cannot tell a crash mid-effect from an effect that never started.",
        "properties": {
            "binding_id": string_schema("The binding acknowledging."),
            "incarnation": string_schema("Its process generation."),
            "fence": {
                "type": "integer",
                "minimum": 0,
                "description": "The generation of the binding it belongs to. A host that was replaced must not be able to close the work of the one that replaced it."
            },
            "delivery_id": string_schema("The item being acknowledged, as the await handed it over."),
            "lease_id": string_schema("The lease from the same await. A foreign or expired one is refused with no effect."),
            "acknowledgement": {
                "type": "string",
                "enum": ["intent", "processed", "failed"],
                "description": "`intent` keeps the lease and records what you are about to do; `processed` closes the item, naming the act; `failed` counts an attempt and may offer it again."
            },
            "action_kind": {
                "type": "string",
                "enum": [
                    "responded",
                    "delegated",
                    "integrated",
                    "correction_requested",
                    "transition_proposed",
                    "escalated_to_user",
                    "no_action"
                ],
                "description": "What you are about to do, or did. Required for `intent` and `processed`."
            },
            "idempotency_key": string_schema("The key of the authorized command you will run, so the intent and the effect read as one pair rather than two unrelated calls."),
            "note": string_schema("What to record alongside it, in your own words."),
            "evidence": string_schema("An evidence reference backing it."),
            "failure_reason": string_schema("Why you could not. Required for `failed`, ignored otherwise.")
        }
    })
}

pub(crate) fn list_attention_deliveries_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "A page of what integrators have been offered, and where each offer stands. Either filter narrows it and both may be given; neither is also allowed, because an operator looking at a deployment that is misbehaving does not yet know which binding to ask about.",
        "properties": {
            "binding_id": string_schema("Only what this binding was offered."),
            "ceremony_id": string_schema("Only what came out of this ceremony."),
            "state": {
                "type": "string",
                "enum": [
                    "queued",
                    "leased",
                    "delivered_to_host",
                    "acknowledged",
                    "processed",
                    "failed",
                    "expired",
                    "superseded"
                ],
                "description": "Only offers in this state. `delivered_to_host` is transport, not processing: it means an activation was accepted, never that anybody acted."
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
    fn asking_for_work_names_the_process_and_the_generation() {
        let schema = await_integrator_attention_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        // Without both, a displaced host cannot be told from the one
        // that displaced it, which is the whole point of the fence.
        assert!(required.contains(&"incarnation"));
        assert!(required.contains(&"fence"));
        assert_eq!(
            schema["properties"]["wait_timeout_ms"]["maximum"],
            json!(MAX_WAIT_MS)
        );
        // The documented revalidation is in the schema a caller reads,
        // not only in the prose it may not.
        assert!(schema["description"]
            .as_str()
            .unwrap()
            .contains("made_get_ceremony_instance"));
    }

    #[test]
    fn acknowledging_names_the_ticket_as_well_as_the_item() {
        let schema = acknowledge_integrator_attention_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert!(required.contains(&"lease_id"));
        assert!(required.contains(&"delivery_id"));
        assert!(required.contains(&"fence"));
        // Intent and effect are two calls, and the schema says which
        // one is being made rather than guessing from the fields.
        assert_eq!(
            schema["properties"]["acknowledgement"]["enum"],
            json!(["intent", "processed", "failed"])
        );
    }

    #[test]
    fn a_scope_is_one_of_two_things_and_says_which() {
        let scope = bind_ceremony_integrator_schema()["properties"]["scope"].clone();
        assert_eq!(
            scope["properties"]["kind"]["enum"],
            json!(["ceremony", "system_execution"])
        );
        assert_eq!(scope["additionalProperties"], json!(false));
    }

    #[test]
    fn the_paperwork_can_be_asked_for_with_no_filter_at_all() {
        let schema = list_attention_deliveries_schema();
        assert!(schema.get("required").is_none());
        assert_eq!(
            schema["properties"]["state"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
    }
}
