//! Request dispatch for the MADE MCP stdio adapter.
//!
//! Parses one JSON-RPC line at a time, routes `initialize` /
//! `tools/list` / `tools/call` to the inner backend, and serializes
//! the response back to stdout. Logs and telemetry happen on the side
//! through [`observability`](crate::observability).

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

#[cfg(feature = "embedded")]
use crate::backend::EMBEDDED_STORE_PATH_ENV;
#[cfg(feature = "grpc")]
use crate::backend::{MadeMcpGrpcTlsConfig, GRPC_ENDPOINT_ENV};
use crate::backend::{MadeMcpToolBackend, MadeMcpToolFuture, MCP_BACKEND_ENV};
#[cfg(feature = "embedded")]
use crate::embedded::EmbeddedMadeMcpBackend;
use crate::fixture::FixtureMadeMcpBackend;
#[cfg(feature = "grpc")]
use crate::grpc::GrpcMadeMcpBackend;
use crate::guidance::{discovery_result, help_result};
use crate::mcp_server_identity::McpServerIdentity;
use crate::observability::{record_tool_error, record_tool_success, ToolErrorKind};
use crate::protocol::{
    initialize_result, is_server_tool, jsonrpc_error, jsonrpc_result, tool_error_result,
    tool_success_result, tools_list_result, DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL,
};

/// Boxed-trait holder over any [`MadeMcpToolBackend`].
pub struct MadeMcpServer {
    backend: Arc<dyn MadeMcpToolBackend>,
    identity: McpServerIdentity,
}

impl Default for MadeMcpServer {
    fn default() -> Self {
        Self::fixture()
    }
}

impl MadeMcpServer {
    /// Fixture-backed server. Returns canned responses; useful for
    /// client wiring without a running MADE.
    #[must_use]
    pub fn fixture() -> Self {
        Self::with_backend(FixtureMadeMcpBackend)
    }

    /// gRPC-backed server with TLS disabled.
    #[cfg(feature = "grpc")]
    #[must_use]
    pub fn grpc(endpoint: impl Into<String>) -> Self {
        Self::grpc_with_tls(endpoint, MadeMcpGrpcTlsConfig::disabled())
    }

    /// gRPC-backed server with a caller-supplied TLS posture.
    #[cfg(feature = "grpc")]
    #[must_use]
    pub fn grpc_with_tls(endpoint: impl Into<String>, tls: MadeMcpGrpcTlsConfig) -> Self {
        Self::with_backend(GrpcMadeMcpBackend::new(endpoint, tls))
    }

    /// In-process ceremony engine with no network service dependency.
    ///
    /// State lives in memory and dies with the process. Use
    /// [`Self::embedded_sqlite`] when ceremonies must survive a restart.
    #[cfg(feature = "embedded")]
    #[must_use]
    pub fn embedded() -> Self {
        Self::with_backend(EmbeddedMadeMcpBackend::default())
    }

    /// Durable in-process ceremony engine backed by one SQLite file.
    ///
    /// Published definitions and running ceremonies are read back from
    /// `path` on start, so a restarted MCP process resumes the sessions a
    /// client already opened instead of silently forgetting them.
    ///
    /// # Errors
    ///
    /// Returns the store's failure when the file cannot be opened —
    /// unreadable path or incompatible database. A durable backend that
    /// cannot reach its state must not degrade into an in-memory one.
    #[cfg(feature = "embedded")]
    pub fn embedded_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let made = made_embedded::EmbeddedMade::open(path).map_err(|error| {
            format!(
                "failed to open the embedded SQLite ceremony store at `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Self::with_backend(EmbeddedMadeMcpBackend::new(made)))
    }

    /// Wrap an arbitrary backend.
    pub fn with_backend(backend: impl MadeMcpToolBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
            identity: McpServerIdentity::default(),
        }
    }

    /// Override the identity advertised to MCP clients.
    #[must_use]
    pub fn with_identity(mut self, identity: McpServerIdentity) -> Self {
        self.identity = identity;
        self
    }

    /// Read backend selection from environment.
    ///
    /// Defaults to `grpc` when compiled, then `embedded`, then
    /// `fixture`. When `grpc` is selected, the endpoint env is
    /// mandatory — no silent fallback to another backend.
    pub fn try_from_env() -> Result<Self, String> {
        let backend = std::env::var(MCP_BACKEND_ENV)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default_backend_name().to_owned());
        #[cfg(feature = "grpc")]
        let endpoint = std::env::var(GRPC_ENDPOINT_ENV).ok();
        #[cfg(feature = "grpc")]
        let tls = MadeMcpGrpcTlsConfig::from_env_for_endpoint(endpoint.as_deref());

        match backend.as_str() {
            #[cfg(feature = "grpc")]
            "grpc" | "live" => {
                let Some(endpoint) = endpoint.filter(|endpoint| !endpoint.trim().is_empty()) else {
                    return Err(format!(
                        "{GRPC_ENDPOINT_ENV} is required when {MCP_BACKEND_ENV}=grpc"
                    ));
                };
                Ok(Self::grpc_with_tls(endpoint, tls))
            }
            #[cfg(feature = "embedded")]
            "embedded" | "in-process" => {
                let path = std::env::var(EMBEDDED_STORE_PATH_ENV)
                    .ok()
                    .filter(|path| !path.trim().is_empty())
                    .ok_or_else(|| {
                        format!(
                            "{EMBEDDED_STORE_PATH_ENV} is required when {MCP_BACKEND_ENV}=embedded"
                        )
                    })?;
                Self::embedded_sqlite(path)
            }
            "fixture" | "fixtures" => Ok(Self::fixture()),
            other => Err(format!(
                "unsupported {MCP_BACKEND_ENV} value `{other}`; compiled backends: {}",
                compiled_backend_names()
            )),
        }
    }

    /// Backend label for the `initialize` response.
    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_name()
    }

    /// TLS posture label for the `initialize` response.
    #[must_use]
    pub fn grpc_tls_mode_name(&self) -> &'static str {
        self.backend.grpc_tls_mode_name()
    }

    /// Handle one JSON-RPC line. Returns `None` when the message was
    /// a notification (no `id`) or when the method is
    /// `notifications/initialized`. Any other case returns one
    /// JSON-RPC response string ready to write to stdout.
    pub async fn handle_json_line(&self, line: &str) -> Option<String> {
        let request = match serde_json::from_str::<Value>(line) {
            Ok(request) => request,
            Err(error) => {
                return Some(jsonrpc_error(
                    Value::Null,
                    -32700,
                    &format!("invalid JSON-RPC message: {error}"),
                ));
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str);

        match method {
            Some("initialize") => id.map(|id| {
                jsonrpc_result(
                    id,
                    initialize_result(
                        self.identity.name(),
                        self.identity.version(),
                        self.backend_name(),
                        self.grpc_tls_mode_name(),
                    ),
                )
            }),
            Some("notifications/initialized") => None,
            Some("tools/list") => id.map(|id| {
                jsonrpc_result(
                    id,
                    tools_list_result(|name| self.backend.supports_tool(name)),
                )
            }),
            Some("tools/call") => match id {
                Some(id) => Some(self.handle_tool_call(id, request.get("params")).await),
                None => None,
            },
            Some(other) => id.map(|id| {
                jsonrpc_error(
                    id,
                    -32601,
                    &format!("unsupported JSON-RPC method `{other}`"),
                )
            }),
            None => Some(jsonrpc_error(
                Value::Null,
                -32600,
                "missing JSON-RPC method",
            )),
        }
    }

    async fn handle_tool_call(&self, id: Value, params: Option<&Value>) -> String {
        let Some(params) = params.and_then(Value::as_object) else {
            return jsonrpc_error(id, -32602, "tools/call requires object params");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return jsonrpc_error(id, -32602, "tools/call requires params.name");
        };
        let arguments = params.get("arguments").unwrap_or(&Value::Null);
        let start = Instant::now();

        let outcome = match name {
            DISCOVER_CAPABILITIES_TOOL => discovery_result(
                self.identity,
                self.backend_name(),
                self.grpc_tls_mode_name(),
                arguments,
                |tool| self.backend.supports_tool(tool),
            )
            .map(tool_success_result),
            GET_HELP_TOOL => help_result(arguments, |tool| self.backend.supports_tool(tool))
                .map(tool_success_result),
            _ => self.backend.call_tool(name, arguments).await,
        };

        match outcome {
            Ok(result) => {
                record_tool_success(
                    self.backend_name(),
                    self.grpc_tls_mode_name(),
                    name,
                    arguments,
                    &result,
                    start.elapsed(),
                );
                jsonrpc_result(id, result)
            }
            Err(message) => {
                record_tool_error(
                    self.backend_name(),
                    self.grpc_tls_mode_name(),
                    name,
                    arguments,
                    if is_server_tool(name) {
                        ToolErrorKind::Validation
                    } else {
                        ToolErrorKind::Backend
                    },
                    &message,
                    start.elapsed(),
                );
                jsonrpc_result(id, tool_error_result(&message))
            }
        }
    }
}

// Allow holding the server's backend behind an Arc directly.
impl<T> MadeMcpToolBackend for Arc<T>
where
    T: MadeMcpToolBackend + ?Sized,
{
    fn backend_name(&self) -> &'static str {
        self.as_ref().backend_name()
    }

    fn grpc_tls_mode_name(&self) -> &'static str {
        self.as_ref().grpc_tls_mode_name()
    }

    fn supports_tool(&self, name: &str) -> bool {
        self.as_ref().supports_tool(name)
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a> {
        self.as_ref().call_tool(name, arguments)
    }
}

fn default_backend_name() -> &'static str {
    if cfg!(feature = "grpc") {
        "grpc"
    } else if cfg!(feature = "embedded") {
        "embedded"
    } else {
        "fixture"
    }
}

fn compiled_backend_names() -> String {
    let mut names = vec!["fixture"];
    if cfg!(feature = "embedded") {
        names.push("embedded");
    }
    if cfg!(feature = "grpc") {
        names.push("grpc");
    }
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_returns_protocol_metadata() {
        let server = MadeMcpServer::fixture();
        let identity = McpServerIdentity::default();
        let response = server
            .handle_json_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await
            .expect("initialize must return a response");
        let parsed: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["serverInfo"]["name"], identity.name());
        assert_eq!(
            parsed["result"]["serverInfo"]["version"],
            identity.version()
        );
        assert_eq!(parsed["result"]["metadata"]["backend"], "fixture");
    }

    #[tokio::test]
    async fn initialize_returns_host_owned_identity() {
        let server =
            MadeMcpServer::fixture().with_identity(McpServerIdentity::new("host-mcp", "9.8.7"));
        let response = server
            .handle_json_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await
            .expect("initialize must return a response");
        let parsed: Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["result"]["serverInfo"]["name"], "host-mcp");
        assert_eq!(parsed["result"]["serverInfo"]["version"], "9.8.7");
    }

    #[tokio::test]
    async fn notifications_initialized_returns_none() {
        let server = MadeMcpServer::fixture();
        assert!(server
            .handle_json_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn tools_list_includes_rpc_and_server_owned_tools() {
        let server = MadeMcpServer::fixture();
        let response = server
            .handle_json_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        let tools = parsed["result"]["tools"].as_array().unwrap();
        // One per RPC plus backend-independent discovery and help.
        assert_eq!(tools.len(), 37);
        assert!(tools
            .iter()
            .any(|tool| tool["name"] == DISCOVER_CAPABILITIES_TOOL));
        assert!(tools.iter().any(|tool| tool["name"] == GET_HELP_TOOL));
    }

    #[tokio::test]
    async fn server_owned_discovery_uses_host_identity_and_active_backend() {
        let server =
            MadeMcpServer::fixture().with_identity(McpServerIdentity::new("host-mcp", "9.8.7"));
        let response = server
            .handle_json_line(
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"made_discover_capabilities","arguments":{}}}"#,
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        let discovery = &parsed["result"]["structuredContent"];

        assert_eq!(discovery["server"]["name"], "host-mcp");
        assert_eq!(discovery["server"]["version"], "9.8.7");
        assert_eq!(discovery["backend"]["name"], "fixture");
        assert!(discovery["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == GET_HELP_TOOL));
    }

    #[tokio::test]
    async fn server_owned_help_returns_user_and_agent_guidance() {
        let server = MadeMcpServer::fixture();
        for audience in ["user", "agent"] {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": GET_HELP_TOOL,
                    "arguments": {"audience": audience}
                }
            });
            let response = server.handle_json_line(&request.to_string()).await.unwrap();
            let parsed: Value = serde_json::from_str(&response).unwrap();
            let help = &parsed["result"]["structuredContent"];
            assert_eq!(help["audience"], audience);
            assert!(help["help_markdown"]
                .as_str()
                .unwrap()
                .starts_with("# made help"));
        }
    }

    #[tokio::test]
    async fn unsupported_method_returns_jsonrpc_error() {
        let server = MadeMcpServer::fixture();
        let response = server
            .handle_json_line(r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#)
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn invalid_json_returns_parse_error() {
        let server = MadeMcpServer::fixture();
        let response = server.handle_json_line("not json").await.unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], -32700);
    }

    #[tokio::test]
    async fn tools_call_with_missing_name_returns_invalid_params() {
        let server = MadeMcpServer::fixture();
        let response = server
            .handle_json_line(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{}}"#)
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], -32602);
    }
}
