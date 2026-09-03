use serde_json::Value;

use super::MadeMcpToolFuture;

/// Single seam between the MCP request dispatcher and any concrete
/// transport.
pub trait MadeMcpToolBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;

    fn grpc_tls_mode_name(&self) -> &'static str {
        "disabled"
    }

    fn supports_tool(&self, name: &str) -> bool {
        crate::protocol::is_grpc_tool(name)
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a>;
}
