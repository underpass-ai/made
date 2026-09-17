use super::{json, string_schema, Value, STRUCT_NUMBER_RULE};
use crate::protocol::{design_pattern_catalog, ROUNDTABLE_FIXED_ORDER_ID};

pub(in crate::protocol) fn ceremony_design_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "objective", "outputs", "participants"],
        "oneOf": design_shape_schema(),
        "x-made-pattern-catalog": design_pattern_catalog(),
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
                "items": stage_schema()
            },
            "pattern": pattern_schema(),
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

fn stage_schema() -> Value {
    json!({
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
            "repeat": repeat_stage_schema(),
            "exit_guards": {
                "type": "array",
                "uniqueItems": true,
                "items": exit_guard_schema(),
                "description": "Additional conditions conjoined with stage completion and any final human approval."
            }
        }
    })
}

fn exit_guard_schema() -> Value {
    json!({
        "oneOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "step", "output_field", "equals"],
                "properties": {
                    "kind": { "const": "output_field" },
                    "step": string_schema("Declared step whose current successful output is inspected."),
                    "output_field": string_schema("Top-level structured output field."),
                    "equals": {
                        "description": format!("Exact JSON value required for this guard. {STRUCT_NUMBER_RULE}")
                    }
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "step"],
                "properties": {
                    "kind": { "const": "step_repeat_exhausted" },
                    "step": string_schema("Repeating step in this stage whose final iteration must be exhausted.")
                }
            }
        ]
    })
}

fn design_shape_schema() -> Value {
    json!([
        {
            "required": ["stages"],
            "not": { "required": ["pattern"] },
            "properties": { "stages": { "minItems": 1 } }
        },
        {
            "required": ["pattern"],
            "properties": { "stages": { "maxItems": 0 } }
        }
    ])
}

fn pattern_schema() -> Value {
    json!({
        "type": "string",
        "enum": [ROUNDTABLE_FIXED_ORDER_ID],
        "description": "Shipped authoring preset. Mutually exclusive with explicit stages; roundtable_fixed_order gives each participant one turn in declaration order, and every turn after the first receives prior contributions."
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
                "description": format!(
                    "Exact JSON value that ends repetition. Missing or unequal output repeats \
                     the stage. {STRUCT_NUMBER_RULE}"
                )
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
