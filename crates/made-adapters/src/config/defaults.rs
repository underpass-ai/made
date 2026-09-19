use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Defaults {
    pub(super) grpc_port: u16,
    pub(super) http_port: u16,
    pub(super) nats_enabled: bool,
    pub(super) nats_url: String,
    pub(super) trigger_subject: String,
    pub(super) publish_prefix: String,
    pub(super) postgres_url: String,
    pub(super) ceremony_store_path: String,
    pub(super) artifact_store_path: String,
    pub(super) memory: String,
    pub(super) grpc_tls_mode: String,
    pub(super) grpc_tls_cert_path: String,
    pub(super) grpc_tls_key_path: String,
    pub(super) grpc_tls_client_ca_path: String,
    pub(super) max_parallel: u8,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            grpc_port: 50055,
            http_port: 8080,
            nats_enabled: true,
            nats_url: "nats://nats:4222".to_owned(),
            trigger_subject: "made.trigger.>".to_owned(),
            publish_prefix: "made".to_owned(),
            postgres_url: String::new(),
            ceremony_store_path: String::new(),
            artifact_store_path: String::new(),
            memory: "automatic".to_owned(),
            grpc_tls_mode: "none".to_owned(),
            grpc_tls_cert_path: String::new(),
            grpc_tls_key_path: String::new(),
            grpc_tls_client_ca_path: String::new(),
            max_parallel: 8,
        }
    }
}
