use serde_json::Value;

use super::{
    MadeMcpAuthorizationSummaryFuture, MadeMcpBackendInitializationFuture, MadeMcpToolFuture,
    ToolTraceContext,
};

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

    /// Which activation adapter the engine behind this backend composed,
    /// as `HostActivationAdapterKind::as_str` spells it.
    ///
    /// Asked rather than declared: whether a host can wait to be woken
    /// is a fact about the composition, and a backend that answered
    /// from a constant would keep saying `none` the day an operator
    /// configured a command. A backend that composes no engine of its
    /// own answers for itself, which is `none`. The word rather than
    /// the value, like `backend_name`, because this seam is compiled
    /// in builds that carry no domain crate at all.
    fn host_activation_adapter(&self) -> &'static str {
        "none"
    }

    /// Which channel this backend records a human guard approval from,
    /// as `HumanApprovalSource::as_str` spells it: `host` when the MCP
    /// session may relay a person's decision, `terminal` when only the
    /// person's own terminal command may record one. A backend that
    /// composes no engine of its own, and every gRPC deployment, answers
    /// `host`.
    fn human_approval_source(&self) -> &'static str {
        "host"
    }

    /// What the principal this backend acts as may do under its
    /// policy, for discovery: the grants that name it, the actions a
    /// live grant covers, and the command a person runs to add one.
    ///
    /// A backend that acts as no principal of its own answers `None`,
    /// which is the default: the fixture, and the gRPC client, where
    /// the service holds the policy and this process is one caller of
    /// it. The protected embedded backend answers for the trusted host
    /// it is, so a session learns which listed tools it will be
    /// refused for before it is refused.
    fn authorization_summary(&self) -> MadeMcpAuthorizationSummaryFuture<'_> {
        Box::pin(async { None })
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
