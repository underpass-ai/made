use made_core::value_objects::HostActivationAdapterKind;
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

    /// Which activation adapter the engine behind this backend composed.
    ///
    /// Asked rather than declared: whether a host can wait to be woken
    /// is a fact about the composition, and a backend that answered
    /// from a constant would keep saying `none` the day an operator
    /// configured a command. A backend that composes no engine of its
    /// own answers for itself, which is `none`.
    fn host_activation_adapter(&self) -> HostActivationAdapterKind {
        HostActivationAdapterKind::None
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
