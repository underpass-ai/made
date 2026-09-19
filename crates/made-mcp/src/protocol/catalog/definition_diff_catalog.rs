use crate::protocol::ceremony_schemas::ceremony_definition_ref_schema;
use crate::protocol::schema_primitives::tool_def;
use crate::protocol::tool_names::DIFF_CEREMONY_DEFINITIONS_TOOL;
use serde_json::{json, Value};

pub(super) fn diff_tool() -> Value {
    tool_def(
        DIFF_CEREMONY_DEFINITIONS_TOOL,
        "Compare two ceremony definitions and say what changed — and, for each change, whether a session already running the earlier one could go on.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["before", "after"],
            "properties": {
                "before": ceremony_definition_ref_schema("The earlier definition."),
                "after": ceremony_definition_ref_schema("The later definition.")
            }
        }),
    )
}
