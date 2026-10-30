use crate::mcp_protocol::protocol_schemas::protocol_definition_ref_schema;
use crate::mcp_protocol::schema_primitives::tool_def;
use crate::mcp_protocol::tool_names::DIFF_PROTOCOL_DEFINITIONS_TOOL;
use serde_json::{json, Value};

pub(super) fn diff_tool() -> Value {
    tool_def(
        DIFF_PROTOCOL_DEFINITIONS_TOOL,
        "Compare two protocol definitions and say what changed — and, for each change, whether a session already running the earlier one could go on.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["before", "after"],
            "properties": {
                "before": protocol_definition_ref_schema("The earlier definition."),
                "after": protocol_definition_ref_schema("The later definition.")
            }
        }),
    )
}
