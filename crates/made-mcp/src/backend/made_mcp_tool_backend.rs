use serde_json::Value;

use super::{MadeMcpBackendInitializationFuture, MadeMcpToolFuture, ToolTraceContext};

/// Single seam between the MCP request dispatcher and any concrete
/// transport.
pub trait MadeMcpToolBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;

    /// Complete backend-specific recovery before the server accepts input.
    fn initialize(&self) -> MadeMcpBackendInitializationFuture<'_> {
        Box::pin(async { Ok(()) })
    }

    fn grpc_tls_mode_name(&self) -> &'static str {
        "disabled"
    }

    fn supports_tool(&self, name: &str) -> bool {
        crate::protocol::is_grpc_tool(name)
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a>;

    fn call_tool_with_trace<'a>(
        &'a self,
        name: &'a str,
        arguments: &'a Value,
        _trace: &'a ToolTraceContext,
    ) -> MadeMcpToolFuture<'a> {
        self.call_tool(name, arguments)
    }
}
