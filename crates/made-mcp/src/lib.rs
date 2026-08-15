//! Stdio MCP adapter for MADE.
//!
//! Exposes MADE capabilities as MCP tools over JSON-RPC 2.0
//! on stdin/stdout. The default backend maps every
//! `underpass.made.v1` RPC to a running service; the optional
//! `embedded` backend executes the ceremony engine in process.
//!
//! See `crates/made-mcp/README.md` for end-user installation, and
//! `docs/operations/mcp-stdio.md` for the canonical UX.

pub mod backend;
#[cfg(feature = "embedded")]
pub mod embedded;
pub mod fixture;
#[cfg(feature = "grpc")]
pub mod grpc;
mod guidance;
pub mod mcp_server_identity;
pub mod observability;
pub mod protocol;
pub mod server;

pub use backend::{
    MadeMcpGrpcTlsConfig, MadeMcpGrpcTlsMode, MadeMcpToolBackend, EMBEDDED_REDB_PATH_ENV,
    GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV,
    GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV, MCP_BACKEND_ENV,
};
#[cfg(feature = "embedded")]
pub use embedded::EmbeddedMadeMcpBackend;
pub use fixture::FixtureMadeMcpBackend;
#[cfg(feature = "grpc")]
pub use grpc::GrpcMadeMcpBackend;
pub use mcp_server_identity::McpServerIdentity;
pub use server::MadeMcpServer;
