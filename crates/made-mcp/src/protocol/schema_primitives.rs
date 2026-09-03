use serde_json::{json, Value};

pub(super) fn attributes_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "description": description
    })
}

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------

#[allow(clippy::needless_pass_by_value)] // json! consumes via macro clone — clippy can't see that
pub(super) fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

pub(super) fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description,
    })
}
