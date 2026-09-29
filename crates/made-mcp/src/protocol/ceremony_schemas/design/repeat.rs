//! Schema fragments for the two repeat policies a design can declare.
//!
//! A stage repeat and a group repeat have different shapes on purpose:
//! a stage inspects its own output, so its condition is flat; a group
//! must also say which of its steps is inspected, so its condition is
//! nested under `until`. Both shapes are published contract. Each
//! schema states its own form and an example in its description, so an
//! author reads the difference before writing either one, and carries
//! a shape hint that the request gate appends to a refusal of that
//! object's fields.

use super::super::{json, string_schema, Value, STRUCT_NUMBER_RULE};

const STAGE_REPEAT_SHAPE: &str = "a stage repeat is flat: \
    {max_iterations, output_field, equals}, e.g. \
    {\"max_iterations\": 3, \"output_field\": \"accepted\", \"equals\": true}; \
    a group repeat nests its condition under `until` instead";

const GROUP_REPEAT_SHAPE: &str = "a group repeat nests its condition as \
    until: {step, output_field, equals}, e.g. {\"max_iterations\": 4, \"until\": \
    {\"step\": \"review_outcome\", \"output_field\": \"outcome\", \"equals\": \"approved\"}}";

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
        "description": "Optional bounded repeat-until policy for this one stage. Flat shape \
            {max_iterations, output_field, equals}, for example \
            {\"max_iterations\": 3, \"output_field\": \"accepted\", \"equals\": true}. \
            Iterations are distinct from technical retry attempts. A group state repeats \
            with the nested group.repeat shape instead.",
        "x-made-shape": STAGE_REPEAT_SHAPE
    })
}

pub(super) fn group_repeat_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["max_iterations", "until"],
        "properties": {
            "max_iterations": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "description": "Hard cap on complete state iterations, including the first."
            },
            "until": {
                "type": "object",
                "additionalProperties": false,
                "required": ["step", "output_field", "equals"],
                "properties": {
                    "step": string_schema("Step in this group whose successful output is inspected after every group iteration."),
                    "output_field": string_schema("Top-level structured output field tested after each group iteration."),
                    "equals": {
                        "description": format!(
                            "Exact JSON value that ends repetition after every group step and its own repeats finish. {STRUCT_NUMBER_RULE}"
                        )
                    }
                },
                "x-made-shape": GROUP_REPEAT_SHAPE
            },
            "on_exhausted": {
                "type": "object",
                "additionalProperties": false,
                "required": ["terminal"],
                "properties": {
                    "terminal": string_schema("Lower_snake identity of the terminal state, for example `exhausted`. Its state id is the upper-case form; it must not name a stage or `completed`.")
                },
                "description": "Optional exit for exhaustion: when the last permitted iteration ends without `until` holding, the group leaves for this terminal through a generated `<group>_repeat_exhausted` transition guarded by `state_repeat_exhausted`. Without it the ceremony stops in the group. Example {\"terminal\": \"exhausted\"}."
            }
        },
        "description": "Optional bounded repeat-until policy for the whole group state. Nested \
            shape {max_iterations, until: {step, output_field, equals}}, for example \
            {\"max_iterations\": 4, \"until\": {\"step\": \"review_outcome\", \
            \"output_field\": \"outcome\", \"equals\": \"approved\"}}. Unlike a stage repeat, \
            the condition sits under `until` because it names which group step is inspected.",
        "x-made-shape": GROUP_REPEAT_SHAPE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_repeat_publishes_its_own_shape_and_an_example() {
        let stage = repeat_stage_schema();
        let group = group_repeat_schema();
        let stage_text = stage["description"].as_str().unwrap();
        let group_text = group["description"].as_str().unwrap();
        assert!(stage_text.contains("{max_iterations, output_field, equals}"));
        assert!(stage_text.contains("\"max_iterations\": 3"));
        assert!(group_text.contains("until: {step, output_field, equals}"));
        assert!(group_text.contains("\"until\": {\"step\": \"review_outcome\""));
        assert_eq!(stage["x-made-shape"], STAGE_REPEAT_SHAPE);
        assert_eq!(group["x-made-shape"], GROUP_REPEAT_SHAPE);
        assert_eq!(
            group["properties"]["until"]["x-made-shape"],
            GROUP_REPEAT_SHAPE
        );
    }
}
