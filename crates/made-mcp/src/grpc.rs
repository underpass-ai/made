//! Live-gRPC backend for the MADE MCP adapter.
//!
//! Talks to a running MADE via the `underpass.made.v1`
//! gRPC contract. Every tool maps 1:1 to one RPC; JSON arguments and
//! responses are translated field-for-field by `json_to_proto` and
//! `proto_to_json`, so the MCP layer never drops or flattens fields.

mod channel;
mod json_to_proto;
mod proto_to_json;
mod status_tool_error;
mod streaming;
mod tools;

use async_trait::async_trait;
use serde_json::Value;
use tonic::transport::Channel;
use tracing::debug;

use crate::backend::{
    endpoint_uri_for_tls_mode, MadeMcpGrpcTlsConfig, MadeMcpToolBackend, MadeMcpToolFuture,
    ToolTraceContext,
};
use crate::protocol::ToolError;

/// What this backend calls itself. Shared with the default lease owner
/// rule, so the id an omitted `lease_owner_id` becomes cannot drift from
/// the name `initialize` advertises.
pub(crate) const GRPC_BACKEND_NAME: &str = "grpc";

/// gRPC-backed implementation of [`MadeMcpToolBackend`].
///
/// Holds a lazily-connected tonic `Channel` (resolved on the first
/// call) and the negotiated TLS posture. The endpoint URI is rewritten
/// to `https://` when TLS is enabled so callers can flip one env var
/// without having to also change the URL scheme.
#[derive(Debug, Clone)]
pub struct GrpcMadeMcpBackend {
    endpoint: String,
    tls: MadeMcpGrpcTlsConfig,
}

impl GrpcMadeMcpBackend {
    /// Build a backend pointed at `endpoint` with the given TLS posture.
    /// No network call happens here — the connection is opened on the
    /// first tool call so `--help`-style probes don't block on DNS.
    pub fn new(endpoint: impl Into<String>, tls: MadeMcpGrpcTlsConfig) -> Self {
        let endpoint = endpoint_uri_for_tls_mode(&endpoint.into(), tls.mode());
        Self { endpoint, tls }
    }

    async fn channel(&self) -> Result<Channel, String> {
        channel::open_channel(&self.endpoint, &self.tls).await
    }
}

#[async_trait]
impl MadeMcpToolBackend for GrpcMadeMcpBackend {
    fn backend_name(&self) -> &'static str {
        GRPC_BACKEND_NAME
    }

    fn grpc_tls_mode_name(&self) -> &'static str {
        self.tls.mode_name()
    }

    fn supports_tool(&self, name: &str) -> bool {
        crate::protocol::is_grpc_tool(name)
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            let trace = ToolTraceContext::for_direct_call(name, arguments);
            self.call_tool_with_trace(name, arguments, &trace).await
        })
    }

    fn call_tool_with_trace<'a>(
        &'a self,
        name: &'a str,
        arguments: &'a Value,
        trace: &'a ToolTraceContext,
    ) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            debug!(
                tool = name,
                tls = self.tls.mode_name(),
                endpoint = self.endpoint.as_str(),
                "made_mcp: dispatching live tool call"
            );
            // A channel that will not open is the engine out of
            // reach, not the engine refusing: waiting is the remedy.
            let channel = self.channel().await.map_err(ToolError::unavailable)?;
            let target_digest = ToolTraceContext::grpc_authorization_target_digest(name, arguments)
                .map_err(ToolError::invalid_request)?;
            let structured = tools::dispatch(
                channel,
                name,
                arguments,
                trace.traceparent(),
                trace.authorization_request_id(),
                &target_digest,
                trace.approval_decision_id(),
            )
            .await?;
            Ok(crate::protocol::tool_success_result(structured))
        })
    }
}
