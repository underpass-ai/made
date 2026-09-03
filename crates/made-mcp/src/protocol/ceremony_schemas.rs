use serde_json::{json, Value};

use super::schema_primitives::{attributes_schema, string_schema};

pub(super) fn ceremony_report_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_ids"],
        "properties": {
            "ceremony_ids": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": string_schema("Identifier of a persisted ceremony instance."),
                "description": "One or more ceremony ids, reported in caller order. Empty lists, duplicates and unknown ids are errors."
            },
            "title": string_schema("Optional report heading. It affects presentation only and is escaped as untrusted Markdown text.")
        }
    })
}

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
            "context": {
                "type": "object",
                "description": "Opening context for the working session.",
                "additionalProperties": true
            }
        }
    })
}

/// Either a published version, named, or a document supplied for the
/// occasion. Both at once has no sensible reading, and the schema says
/// so rather than leaving the server to discover it.
pub(super) fn ceremony_definition_ref_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": description,
        "properties": {
            "ceremony": string_schema("Name of a published definition. Give this with `version`."),
            "version": string_schema("Version of a published definition. Give this with `ceremony`."),
            "definition_yaml": string_schema("A definition supplied for the comparison, instead of naming a published one.")
        }
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

pub(super) fn ceremony_design_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "objective", "outputs", "participants", "stages"],
        "properties": {
            "name": string_schema("Stable lower_snake_case identity for the designed ceremony."),
            "version": string_schema("Immutable publication version. Defaults to 1.0."),
            "objective": string_schema("The single question or artifact this ceremony exists to resolve or produce."),
            "required_inputs": unique_string_array_schema("Context keys every run must provide."),
            "optional_inputs": unique_string_array_schema("Context keys a run may provide."),
            "outputs": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": { "type": "string", "minLength": 1 },
                "description": "Named output objects the completed ceremony promises."
            },
            "participants": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["role_id"],
                    "properties": {
                        "role_id": string_schema("Role seated at the working session."),
                        "capabilities": {
                            "type": "array",
                            "uniqueItems": true,
                            "items": {
                                "type": "string",
                                "enum": ["request_intervention", "respond_to_intervention"]
                            },
                            "description": "Optional live-agenda capabilities beyond owned stages."
                        }
                    }
                }
            },
            "stages": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "owner_role_id", "instructions"],
                    "properties": {
                        "id": string_schema("Lower_snake_case step identity. Declaration order is execution order."),
                        "owner_role_id": string_schema("Participant role allowed to run this stage."),
                        "instructions": string_schema("Concrete instructions and success criteria for this stage."),
                        "handler": string_schema("Host step-handler specialty. Defaults to host_callback."),
                        "see_prior": {
                            "type": "boolean",
                            "description": "Whether earlier stage outputs enter this stage; defaults to false for the first stage and true afterwards."
                        },
                        "num_agents": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Council size. Defaults to one."
                        },
                        "review_rounds": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "Adversarial peer-review rounds. A positive value requires at least two agents."
                        },
                        "repeat": repeat_stage_schema()
                    }
                }
            },
            "final_approval": {
                "type": "object",
                "additionalProperties": false,
                "required": ["role_id"],
                "properties": {
                    "role_id": string_schema("Participant role whose explicit human approval unlocks completion."),
                    "guard_name": string_schema("Human guard identity. Defaults to human_approved_outcome."),
                    "trigger": string_schema("Final transition trigger. Defaults to approve_outcome.")
                },
                "description": "Optional explicit human gate after the final stage. Designing it never records approval."
            },
            "step_timeout_seconds": {
                "type": "integer",
                "minimum": 1,
                "description": "Default step timeout written into the draft. Defaults to 300."
            },
            "max_attempts": {
                "type": "integer",
                "minimum": 1,
                "description": "Default maximum attempts written into the draft. Defaults to two."
            },
            "backoff_seconds": {
                "type": "integer",
                "minimum": 0,
                "description": "Default retry backoff written into the draft. Defaults to one."
            }
        }
    })
}

pub(super) fn repeat_stage_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["max_iterations", "output_field", "equals"],
        "properties": {
            "max_iterations": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "description": "Hard cap on semantic executions of this stage, including the first."
            },
            "output_field": string_schema("Top-level structured step-output field tested after each successful iteration."),
            "equals": {
                "description": "Exact JSON value that ends repetition. Missing or unequal output repeats the stage."
            }
        },
        "description": "Optional bounded repeat-until policy. Iterations are distinct from technical retry attempts."
    })
}

pub(super) fn unique_string_array_schema(description: &str) -> Value {
    json!({
        "type": "array",
        "uniqueItems": true,
        "items": { "type": "string", "minLength": 1 },
        "description": description,
    })
}

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
            "context": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque initial ceremony context forwarded to guards and handlers."
            },
            "lease_owner_id": string_schema("Optional logical runner acquiring step leases. The server applies a default when omitted."),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": "Step lease TTL in milliseconds. Zero or omitted uses the server default."
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
            "context": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque initial ceremony context forwarded to guards and handlers."
            }
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
            "lease_owner_id": string_schema("Optional logical runner acquiring the step lease."),
            "idempotency_key": string_schema("Optional unique execution key. The server mints one when omitted."),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": "Step lease TTL in milliseconds. Zero or omitted uses the server default."
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
            "lease_owner_id": string_schema("Logical host runner acquiring the step lease."),
            "idempotency_key": string_schema("Unique execution key for this claim. The server mints one when omitted."),
            "lease_ttl_ms": {
                "type": "integer",
                "minimum": 0,
                "description": "Lease TTL in milliseconds. Zero or omitted uses the five-minute external-host default."
            }
        }
    })
}

pub(super) fn complete_ceremony_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ceremony_id", "step_id", "actor_kind", "status"],
        "properties": {
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
                "uniqueItems": true,
                "items": { "type": "string", "minLength": 1 },
                "description": "Concrete conditions that would make it appropriate to ask again."
            }
        }
    })
}

/// Something this session produced that a reason can point at.
pub(super) fn ceremony_record_ref_schema(description: &str) -> Value {
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

pub(super) fn ceremony_reason_schema() -> Value {
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
                    "chosen_because",
                    "achieved_by",
                    "follows_from",
                    "satisfies_constraint",
                    "violates_constraint",
                    "supersedes",
                    "contradicts"
                ],
                "description": "How the first came from the second. `achieved_by` is the how, and it is what turns a resolved session from a precedent into a procedure. `answers` is absent: it states the shape of the session rather than anyone's judgement, and only the engine asserts it."
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

pub(super) fn request_ceremony_intervention_schema() -> Value {
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
                "minItems": 1,
                "uniqueItems": true,
                "items": { "type": "string", "minLength": 1 },
                "description": "Optional responding roles. Omit to address the whole table."
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
            }
        }
    })
}

pub(super) fn respond_to_ceremony_intervention_schema() -> Value {
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
            "details": attributes_schema("Structured response context or evidence references.")
        }
    })
}

pub(super) fn close_ceremony_intervention_schema() -> Value {
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

pub(super) fn collect_ceremony_evidence_schema() -> Value {
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
