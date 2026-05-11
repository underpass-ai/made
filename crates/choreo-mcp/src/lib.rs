//! Stdio MCP adapter for the Underpass Choreographer.
//!
//! Exposes every RPC in `underpass.choreo.v1` as an MCP tool over
//! JSON-RPC 2.0 on stdin/stdout. Designed so coding agents (Codex,
//! Claude Desktop) can talk to a running choreographer without
//! re-implementing gRPC.
//!
//! See `crates/choreo-mcp/README.md` for end-user installation, and
//! `docs/operations/mcp-stdio.md` for the canonical UX.

pub mod backend;
pub mod fixture;
pub mod grpc;
pub mod observability;
pub mod protocol;
pub mod server;

pub use backend::{
    ChoreoMcpGrpcTlsConfig, ChoreoMcpGrpcTlsMode, ChoreoMcpToolBackend, GRPC_ENDPOINT_ENV,
    GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV, GRPC_TLS_KEY_PATH_ENV,
    GRPC_TLS_MODE_ENV, MCP_BACKEND_ENV,
};
pub use fixture::FixtureChoreoMcpBackend;
pub use grpc::GrpcChoreoMcpBackend;
pub use server::ChoreoMcpServer;
