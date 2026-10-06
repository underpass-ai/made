//! Stdio MCP adapter for MADE.
//!
//! Exposes MADE capabilities as MCP tools over JSON-RPC 2.0
//! on stdin/stdout. The default backend maps every
//! `underpass.made.v1` RPC to a running service; the optional
//! `embedded` backend executes the ceremony engine in process.
//!
//! See `crates/made-mcp/README.md` for end-user installation, and
//! `docs/operations/mcp-stdio.md` for the canonical UX.

#[cfg(feature = "embedded")]
pub mod approve_guard_command;
pub mod authorization_channel;
pub mod backend;
#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "embedded")]
pub mod evidence_command;
pub mod fixture;
#[cfg(feature = "embedded")]
pub mod grant_command;
#[cfg(feature = "grpc")]
pub mod grpc;
mod guidance;
pub mod human_approval_source;
pub mod mcp_server_identity;
#[cfg(feature = "embedded")]
pub mod migrate_store;
pub mod observability;
pub mod protocol;
mod renderers;
pub mod server;
pub mod terminal;
pub mod tool_profile;

pub use authorization_channel::{GRANT_COMMAND, GRANT_SCRIPT};
pub use backend::{
    MadeMcpGrpcTlsConfig, MadeMcpGrpcTlsMode, MadeMcpToolBackend, EMBEDDED_STORE_PATH_ENV,
    EVENT_SINK_PATH_ENV, GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV,
    GRPC_TLS_DOMAIN_NAME_ENV, GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV, MCP_BACKEND_ENV,
};
#[cfg(feature = "embedded")]
pub use embedded::EmbeddedMadeMcpBackend;
pub use fixture::FixtureMadeMcpBackend;
#[cfg(feature = "grpc")]
pub use grpc::GrpcMadeMcpBackend;
pub use human_approval_source::{HumanApprovalSource, HUMAN_APPROVAL_SOURCE_ENV};
pub use mcp_server_identity::McpServerIdentity;
#[cfg(feature = "embedded")]
pub use migrate_store::{migrate_store, migrate_store_report, MigrateStoreOutcome};
pub use server::MadeMcpServer;
pub use tool_profile::{ToolProfile, TOOL_PROFILE_ENV};
