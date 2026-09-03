use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Tool-call result shape (MCP spec)
// ---------------------------------------------------------------------------

/// MCP success result: `content[].text` for human consumers +
/// `structuredContent` for machine consumers + `isError: false`.
#[allow(clippy::needless_pass_by_value)] // structured is used twice; consumed by json!
pub(crate) fn tool_success_result(structured: Value) -> Value {
    let text = serde_json::to_string_pretty(&structured).expect("structured JSON should serialize");
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false,
    })
}

/// MCP tool error: spec says `isError: true` in the *tool result*,
/// **not** as a JSON-RPC `error`.
pub(crate) fn tool_error_result(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

// ---------------------------------------------------------------------------
// JSON-RPC framing
// ---------------------------------------------------------------------------

#[allow(clippy::needless_pass_by_value)] // both args consumed by json!
pub(crate) fn jsonrpc_result(id: Value, result: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
    .to_string()
}

#[allow(clippy::needless_pass_by_value)] // id consumed by json!
pub(crate) fn jsonrpc_error(id: Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}
