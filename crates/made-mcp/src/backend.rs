//! Backend trait + env-driven TLS configuration for the MADE MCP
//! server.
//!
//! The MCP layer talks to exactly one [`MadeMcpToolBackend`]; the
//! production impl is gRPC against a running MADE, the
//! embedded impl runs the ceremony engine in process, and the fixture
//! impl reads canned responses for client-wiring smoke tests.
//! Backend selection happens at startup from
//! [`MADE_MCP_BACKEND`](MCP_BACKEND_ENV) — default `grpc`,
//! fail-fast when the endpoint env is missing.

/// Endpoint URL the MCP gRPC backend should connect to.
pub const GRPC_ENDPOINT_ENV: &str = "MADE_MCP_GRPC_ENDPOINT";
/// Backend selector: `grpc` (default), `embedded`, or `fixture`.
pub const MCP_BACKEND_ENV: &str = "MADE_MCP_BACKEND";
/// redb database file the durable embedded backend opens. Required when
/// the embedded backend is selected: where ceremony state survives a
/// restart is an operator decision, never a default this crate invents.
pub const EMBEDDED_REDB_PATH_ENV: &str = "MADE_MCP_REDB_PATH";

/// Which engine a **new** embedded store is created with: `redb` (default)
/// or `sqlite`.
///
/// It decides nothing about an existing store. A ceremony store announces
/// its own engine in its first bytes, so one that already exists is always
/// opened by whatever wrote it; asking for a different engine is refused
/// rather than quietly ignored.
pub const EMBEDDED_ENGINE_ENV: &str = "MADE_MCP_ENGINE";
/// Optional pre-rename Choreographer redb source. When set for the embedded
/// backend, startup imports it read-only into [`EMBEDDED_REDB_PATH_ENV`].
pub const LEGACY_REDB_PATH_ENV: &str = "MADE_MCP_LEGACY_REDB_PATH";
/// TLS mode override for the gRPC client: `disabled`/`server`/`mutual`.
pub const GRPC_TLS_MODE_ENV: &str = "MADE_MCP_GRPC_TLS_MODE";
/// PEM bundle the client should trust as a CA when verifying the
/// server (server or mutual mode).
pub const GRPC_TLS_CA_PATH_ENV: &str = "MADE_MCP_GRPC_TLS_CA_PATH";
/// Client certificate PEM the MCP presents to the server (mutual).
pub const GRPC_TLS_CERT_PATH_ENV: &str = "MADE_MCP_GRPC_TLS_CERT_PATH";
/// Client private key PEM matching `_CERT_PATH` (mutual).
pub const GRPC_TLS_KEY_PATH_ENV: &str = "MADE_MCP_GRPC_TLS_KEY_PATH";
/// Override the TLS SNI/domain (when the URL host differs from the
/// cert CN/SAN, e.g. behind a kube Service).
pub const GRPC_TLS_DOMAIN_NAME_ENV: &str = "MADE_MCP_GRPC_TLS_DOMAIN_NAME";

mod made_mcp_grpc_tls_config;
mod made_mcp_grpc_tls_mode;
mod made_mcp_tool_backend;
mod made_mcp_tool_future;

pub use made_mcp_grpc_tls_config::MadeMcpGrpcTlsConfig;
pub use made_mcp_grpc_tls_mode::MadeMcpGrpcTlsMode;
pub use made_mcp_tool_backend::MadeMcpToolBackend;
pub use made_mcp_tool_future::MadeMcpToolFuture;

/// When TLS is enabled, automatically rewrite an `http://` endpoint to
/// `https://` so callers can flip a single env var (the TLS knob)
/// without having to also change the URL scheme.
#[cfg(any(feature = "grpc", test))]
pub(crate) fn endpoint_uri_for_tls_mode(endpoint: &str, mode: MadeMcpGrpcTlsMode) -> String {
    if mode == MadeMcpGrpcTlsMode::Disabled {
        return endpoint.to_string();
    }
    endpoint.strip_prefix("http://").map_or_else(
        || endpoint.to_string(),
        |without_scheme| format!("https://{without_scheme}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_uri_upgrades_http_when_tls_enabled() {
        assert_eq!(
            endpoint_uri_for_tls_mode("http://127.0.0.1:50055", MadeMcpGrpcTlsMode::Server),
            "https://127.0.0.1:50055"
        );
        assert_eq!(
            endpoint_uri_for_tls_mode("https://x.example", MadeMcpGrpcTlsMode::Mutual),
            "https://x.example"
        );
        assert_eq!(
            endpoint_uri_for_tls_mode("http://127.0.0.1:50055", MadeMcpGrpcTlsMode::Disabled),
            "http://127.0.0.1:50055"
        );
    }
}
