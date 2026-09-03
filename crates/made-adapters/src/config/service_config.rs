use serde::{Deserialize, Serialize};

use super::GrpcTlsConfig;

/// Validated process configuration consumed by the deployable composition root.
///
/// This is deliberately an adapter DTO rather than a domain value. Ports, bind
/// addresses and persistence URLs describe how the service is deployed; they
/// are not concepts the MADE domain needs in order to deliberate or run a
/// ceremony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub grpc_port: u16,
    pub http_port: u16,
    pub nats_enabled: bool,
    pub nats_url: String,
    pub trigger_subject: String,
    pub publish_prefix: String,
    pub postgres_url: Option<String>,
    pub ceremony_store_path: Option<String>,
    pub grpc_tls: GrpcTlsConfig,
}
