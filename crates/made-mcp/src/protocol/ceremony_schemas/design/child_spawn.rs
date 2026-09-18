//! Schema fragment for durable child ceremony spawning.

use super::super::{json, string_schema, Value};

pub(super) fn child_spawn_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["children", "max_children", "max_depth"],
        "properties": {
            "children": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ceremony", "version"],
                    "properties": {
                        "ceremony": string_schema("Published child ceremony name."),
                        "version": string_schema("Published child ceremony version."),
                        "inputs": {
                            "type": "object",
                            "additionalProperties": { "type": "string" },
                            "description": "Child context key to sealed parent context key binding."
                        }
                    }
                }
            },
            "max_children": { "type": "integer", "minimum": 1 },
            "max_depth": { "type": "integer", "minimum": 1 }
        }
    })
}
