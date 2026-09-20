//! Request dispatch for the MADE MCP stdio adapter.
//!
//! Parses one JSON-RPC line at a time, routes `initialize` /
//! `tools/list` / `tools/call` to the inner backend, and serializes
//! the response back to stdout. Logs and telemetry happen on the side
//! through [`observability`](crate::observability).

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use crate::backend::{
    MadeMcpBackendInitializationFuture, MadeMcpToolBackend, MadeMcpToolFuture, ToolTraceContext,
    MCP_BACKEND_ENV,
};
#[cfg(feature = "grpc")]
use crate::backend::{MadeMcpGrpcTlsConfig, GRPC_ENDPOINT_ENV};
#[cfg(feature = "embedded")]
use crate::backend::{EMBEDDED_STORE_PATH_ENV, EVENT_SINK_PATH_ENV};
#[cfg(feature = "embedded")]
use crate::embedded::EmbeddedMadeMcpBackend;
use crate::fixture::FixtureMadeMcpBackend;
#[cfg(feature = "grpc")]
use crate::grpc::GrpcMadeMcpBackend;
use crate::guidance::{discovery_result, help_result};
use crate::mcp_server_identity::McpServerIdentity;
use crate::observability::{record_tool_error, record_tool_success, ToolErrorKind};
use crate::protocol::{
    initialize_result, jsonrpc_error, jsonrpc_result, normalise_numbers, tool_error_result,
    tool_success_result, tools_list_result, validate_tool_request, ToolError, ToolErrorCode,
    DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL,
};

#[cfg(feature = "embedded")]
mod embedded_step_continuation;

#[cfg(feature = "embedded")]
const AUTH_POLICY_ID_ENV: &str = "MADE_AUTH_POLICY_ID";
#[cfg(feature = "embedded")]
const AUTH_TRUSTED_HOST_ID_ENV: &str = "MADE_AUTH_TRUSTED_HOST_ID";

#[cfg(feature = "embedded")]
fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required for the protected embedded backend"))
}

/// Boxed-trait holder over any [`MadeMcpToolBackend`].
pub struct MadeMcpServer {
    backend: Arc<dyn MadeMcpToolBackend>,
    identity: McpServerIdentity,
    process_session_namespace: String,
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
        let metrics = Arc::new(
            made_adapters::metrics::PrometheusMetricsRecorder::new()
                .map_err(|error| format!("failed to initialize embedded metrics: {error}"))?,
        );
        let sink = std::env::var(EVENT_SINK_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|sink_path| {
                made_adapters::event_sink::JsonLinesCeremonyEventSink::open_with_metrics(
                    sink_path,
                    metrics.clone(),
                )
            })
            .transpose()
            .map_err(|error| format!("failed to open {EVENT_SINK_PATH_ENV}: {error}"))?;
        let made = match sink {
            Some(sink) => {
                made_embedded::EmbeddedMade::open_with_observability(path, metrics, Arc::new(sink))
            }
            None => made_embedded::EmbeddedMade::open_with_metrics(path, metrics),
        }
        .map_err(|error| {
            format!(
                "failed to open the embedded SQLite ceremony store at `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Self::with_backend(EmbeddedMadeMcpBackend::new(made)))
    }

    #[cfg(feature = "embedded")]
    fn embedded_sqlite_authorized(
        path: impl AsRef<std::path::Path>,
        policy_id: &str,
        trusted_host_id: &str,
    ) -> Result<Self, String> {
        use made_adapters::artifacts::LocalArtifactStore;
        use made_adapters::clock::SystemClock;
        use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
        use made_app::authorization::{
            AuthorizeOperationUseCase, ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate,
        };
        use made_core::ports::{
            ArtifactStorePort, AuthorizationPolicyStorePort, ExecutionReceiptStorePort,
        };
        use made_core::value_objects::{
            AuthenticatedPrincipal, AuthenticationMethod, AuthorizationDecisionTtl,
            AuthorizationPolicyId, PrincipalId, PrincipalKind,
        };

        let path = path.as_ref();
        let metrics = Arc::new(
            made_adapters::metrics::PrometheusMetricsRecorder::new()
                .map_err(|error| format!("failed to initialize embedded metrics: {error}"))?,
        );
        let sink = std::env::var(EVENT_SINK_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|sink_path| {
                made_adapters::event_sink::JsonLinesCeremonyEventSink::open_with_metrics(
                    sink_path,
                    metrics.clone(),
                )
            })
            .transpose()
            .map_err(|error| format!("failed to open {EVENT_SINK_PATH_ENV}: {error}"))?;
        let made = match sink {
            Some(sink) => {
                made_embedded::EmbeddedMade::open_with_observability(path, metrics, Arc::new(sink))
            }
            None => made_embedded::EmbeddedMade::open_with_metrics(path, metrics),
        }
        .map_err(|error| {
            format!(
                "failed to open the embedded SQLite ceremony store at `{}`: {error}",
                path.display()
            )
        })?;
        let store: Arc<dyn AuthorizationPolicyStorePort> =
            Arc::new(SqliteAuthorizationPolicyStore::open(path).map_err(|error| {
                format!("failed to open embedded authorization store: {error}")
            })?);
        let policy_id = AuthorizationPolicyId::new(policy_id).map_err(|error| error.to_string())?;
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new(trusted_host_id).map_err(|error| error.to_string())?,
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .map_err(|error| error.to_string())?;
        let clock = Arc::new(SystemClock::new());
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            policy_id.clone(),
            store.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(60).expect("fixed TTL is valid"),
        ));
        let gate = TrustedHostAuthorizationGate::new(authorize, principal)
            .map_err(|error| error.to_string())?;
        let read_policy = ReadAuthorizationPolicyUseCase::new(policy_id.clone(), store.clone());
        let (step_continuation, ceremony_store) =
            embedded_step_continuation::wire(path, policy_id.clone(), store.clone(), clock)?;
        let made = made.with_authorization_policy(policy_id, store);
        let receipts: Arc<dyn ExecutionReceiptStorePort> = ceremony_store;
        let mut artifact_root = path.as_os_str().to_owned();
        artifact_root.push(".artifacts");
        let artifacts: Arc<dyn ArtifactStorePort> = Arc::new(
            LocalArtifactStore::open(std::path::PathBuf::from(artifact_root)).map_err(|error| {
                format!("failed to open artifact authorization resolver: {error}")
            })?,
        );
        Ok(Self::with_backend(
            EmbeddedMadeMcpBackend::with_authorization(
                made,
                gate,
                read_policy,
                step_continuation,
                artifacts,
                receipts,
            ),
        ))
    }

    /// Wrap an arbitrary backend.
    pub fn with_backend(backend: impl MadeMcpToolBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
            identity: McpServerIdentity::default(),
            process_session_namespace: uuid::Uuid::new_v4().simple().to_string(),
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
                let policy_id = required_env(AUTH_POLICY_ID_ENV)?;
                let trusted_host_id = required_env(AUTH_TRUSTED_HOST_ID_ENV)?;
                Self::embedded_sqlite_authorized(path, &policy_id, &trusted_host_id)
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

    /// Complete recovery required by the selected backend before serving.
    pub async fn initialize_backend(&self) -> Result<(), String> {
        self.backend.initialize().await
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
        let received = params.get("arguments").unwrap_or(&Value::Null);
        let supplied_traceparent = params
            .get("_meta")
            .and_then(Value::as_object)
            .and_then(|meta| meta.get("traceparent"))
            .and_then(Value::as_str)
            .or_else(|| {
                received
                    .get("_meta")
                    .and_then(Value::as_object)
                    .and_then(|meta| meta.get("traceparent"))
                    .and_then(Value::as_str)
            });
        let supplied_request_namespace = request_namespace(params, received);
        let supplied_approval_decision = approval_decision(params, received);
        let start = Instant::now();

        // Two things happen to a call before a backend sees it, and
        // both happen here so that both happen identically whichever
        // engine is mounted.
        //
        // First the numbers are read by one rule, because the contract
        // carries an open payload as `google.protobuf.Struct` and a
        // `Struct` cannot tell `1` from `1.0`: settled at ingress, or
        // settled by whichever engine answered (issue #75).
        //
        // Then the published schema decides what is acceptable, and
        // hands back the arguments it accepted: an unset optional
        // written as `null` is dropped and the host's own `_`-prefixed
        // keys are left behind, so no request mapper needs an opinion
        // about either. The engine is never reached, so the refusal is
        // worded by this layer rather than by whichever mapper looked
        // first, and everything either step reports is the call's own
        // fault (ADR-014, plan §3.6 F4).
        let accepted = normalise_numbers(received).and_then(|arguments| {
            validate_tool_request(name, &arguments, |tool| self.backend.supports_tool(tool))
        });
        // What is recorded is what ran; a call that never ran is
        // recorded as the client wrote it.
        let arguments = accepted.as_ref().unwrap_or(received);
        let trace = match ToolTraceContext::for_invocation(
            supplied_traceparent,
            &self.process_session_namespace,
            &id,
            name,
            arguments,
            supplied_request_namespace,
            supplied_approval_decision,
        ) {
            Ok(trace) => trace,
            Err(error) => {
                return jsonrpc_result(id, tool_error_result(&ToolError::invalid_request(error)));
            }
        };
        // The two server-owned tools answer about this process and
        // reach no engine, so the only way either can fail is the call
        // itself: an unknown field, a missing one, an audience that is
        // not one of the two. One place says so, on both backends.
        let outcome = match &accepted {
            Err(error) => Err(error.clone()),
            Ok(arguments) => match name {
                DISCOVER_CAPABILITIES_TOOL => discovery_result(
                    self.identity,
                    self.backend_name(),
                    self.grpc_tls_mode_name(),
                    arguments,
                    |tool| self.backend.supports_tool(tool),
                )
                .map(tool_success_result)
                .map_err(ToolError::invalid_request),
                GET_HELP_TOOL => help_result(arguments, |tool| self.backend.supports_tool(tool))
                    .map(tool_success_result)
                    .map_err(ToolError::invalid_request),
                _ => {
                    self.backend
                        .call_tool_with_trace(name, arguments, &trace)
                        .await
                }
            },
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
            Err(error) => {
                record_tool_error(
                    self.backend_name(),
                    self.grpc_tls_mode_name(),
                    name,
                    arguments,
                    if error.code() == ToolErrorCode::InvalidRequest {
                        ToolErrorKind::Validation
                    } else {
                        ToolErrorKind::Backend
                    },
                    &error.to_string(),
                    start.elapsed(),
                );
                jsonrpc_result(id, tool_error_result(&error))
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

    fn initialize(&self) -> MadeMcpBackendInitializationFuture<'_> {
        self.as_ref().initialize()
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

    fn call_tool_with_trace<'a>(
        &'a self,
        name: &'a str,
        arguments: &'a Value,
        trace: &'a ToolTraceContext,
    ) -> MadeMcpToolFuture<'a> {
        self.as_ref().call_tool_with_trace(name, arguments, trace)
    }
}

fn request_namespace<'a>(
    params: &'a serde_json::Map<String, Value>,
    arguments: &'a Value,
) -> Option<&'a str> {
    params
        .get("_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("made_request_id"))
        .and_then(Value::as_str)
        .or_else(|| {
            arguments
                .get("_meta")
                .and_then(Value::as_object)
                .and_then(|meta| meta.get("made_request_id"))
                .and_then(Value::as_str)
        })
}

fn approval_decision<'a>(
    params: &'a serde_json::Map<String, Value>,
    arguments: &'a Value,
) -> Option<&'a str> {
    params
        .get("_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("made_approval_decision_id"))
        .and_then(Value::as_str)
        .or_else(|| {
            arguments
                .get("_meta")
                .and_then(Value::as_object)
                .and_then(|meta| meta.get("made_approval_decision_id"))
                .and_then(Value::as_str)
        })
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
        assert_eq!(tools.len(), 99);
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
