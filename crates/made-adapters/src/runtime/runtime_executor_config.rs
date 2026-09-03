use super::{RuntimeClientTlsConfig, RuntimePrincipal};

/// Configuration for the Runtime executor adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutorConfig {
    pub grpc_endpoint: String,
    pub principal: RuntimePrincipal,
    pub tls: RuntimeClientTlsConfig,
}
