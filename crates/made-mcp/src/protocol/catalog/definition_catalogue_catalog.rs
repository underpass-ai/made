//! Reading the published catalogue back, as a client discovers it.

use crate::protocol::schema_primitives::{string_schema, tool_def};
use crate::protocol::tool_names::{GET_CEREMONY_DEFINITION_TOOL, LIST_CEREMONY_DEFINITIONS_TOOL};
use serde_json::{json, Value};

pub(super) fn list_tool() -> Value {
    tool_def(
        LIST_CEREMONY_DEFINITIONS_TOOL,
        "List the ceremony definitions that have been published: name, version, digest, description and how many states and steps each declares. Ordered by name then version; bounded and resumable with the `next_cursor` it returns. Only published versions — a definition merely mounted in one host process is not in the catalogue. Read-only.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "ceremony": string_schema("Only the versions published under this name. Omitted lists every name."),
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 50},
                "cursor": string_schema("The `next_cursor` of the previous page, written name@version.")
            }
        }),
    )
}

pub(super) fn get_tool() -> Value {
    tool_def(
        GET_CEREMONY_DEFINITION_TOOL,
        "Read one published ceremony definition back: the authoring YAML that publish, validate and diff accept, and the digest it was published with. The YAML parses back to that digest; a definition that could not be written so is refused rather than approximated. A version nobody published is not found. Read-only.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["ceremony", "version"],
            "properties": {
                "ceremony": string_schema("Name the definition was published under."),
                "version": string_schema("Version to read.")
            }
        }),
    )
}
