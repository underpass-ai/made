//! Shared request-mapping error classification for gRPC-backed tools.

use crate::protocol::ToolError;

/// Request mappers only report faults in the caller's JSON arguments.
pub(super) fn bad_request(message: String) -> ToolError {
    ToolError::invalid_request(message)
}
