//! Tracing + OTel records for every MCP tool call.
//!
//! Discipline: tool error messages may contain user payload (an
//! invalid argument echoes part of the request). We never put them
//! into metrics raw — they get content-hashed first. The full message
//! goes to the tool result text (where the caller wanted it) and to
//! structured tracing (where the operator opted in via log levels).

use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

/// Coarse error taxonomy for metrics. Stays a small enum so charts
/// have a finite label set.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ToolErrorKind {
    /// Backend (gRPC / fixture) returned an error.
    Backend,
    /// A server-owned tool rejected invalid arguments.
    Validation,
}

impl ToolErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Validation => "validation",
        }
    }
}

/// Record a successful tool call. Captures duration + size hints of
/// the request and response so operators can spot pathological tool
/// shapes (huge requests, empty responses) without seeing the payload.
pub(crate) fn record_tool_success(
    backend: &str,
    grpc_tls: &str,
    tool: &str,
    arguments: &Value,
    result: &Value,
    duration: Duration,
) {
    debug!(
        backend,
        grpc_tls,
        tool,
        status = "success",
        duration_ms = duration.as_millis() as u64,
        args_size = approx_size(arguments),
        result_size = approx_size(result),
        "choreo_mcp_tool"
    );
}

/// Record a failed tool call. The user-visible `message` is **not**
/// emitted raw — only its SHA-256 prefix, so metrics never leak
/// payload fragments.
pub(crate) fn record_tool_error(
    backend: &str,
    grpc_tls: &str,
    tool: &str,
    arguments: &Value,
    kind: ToolErrorKind,
    message: &str,
    duration: Duration,
) {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();
    let mut error_hash = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        let _ = write!(&mut error_hash, "{byte:02x}");
    }
    warn!(
        backend,
        grpc_tls,
        tool,
        status = "error",
        error_kind = kind.as_str(),
        error_hash,
        duration_ms = duration.as_millis() as u64,
        args_size = approx_size(arguments),
        "choreo_mcp_tool"
    );
}

fn approx_size(value: &Value) -> u64 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 8,
        Value::String(s) => s.len() as u64,
        Value::Array(arr) => arr.iter().map(approx_size).sum(),
        Value::Object(obj) => obj
            .iter()
            .map(|(k, v)| (k.len() as u64).saturating_add(approx_size(v)))
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_labels_are_stable() {
        assert_eq!(ToolErrorKind::Backend.as_str(), "backend");
        assert_eq!(ToolErrorKind::Validation.as_str(), "validation");
    }

    #[test]
    fn approx_size_is_recursive() {
        let v = serde_json::json!({
            "a": "hi",
            "b": [1, 2, 3],
            "c": { "d": "x" }
        });
        // Order: keys + values. "hi"=2, [1,2,3]=24, "x"=1 plus key bytes.
        // Not a strict guarantee — just smoke that it's >0 and stable.
        assert!(approx_size(&v) > 0);
        assert_eq!(approx_size(&Value::Null), 0);
        assert_eq!(approx_size(&serde_json::json!("hi")), 2);
    }
}
