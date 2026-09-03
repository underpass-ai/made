use serde::{Deserialize, Serialize};

/// Validated transport-security configuration for the deployable gRPC adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrpcTlsConfig {
    /// Plain HTTP/2 over TCP.
    Disabled,
    /// One-way TLS: the server presents an identity.
    Server { cert_path: String, key_path: String },
    /// Mutual TLS: the client must present an identity signed by this CA.
    Mutual {
        cert_path: String,
        key_path: String,
        client_ca_path: String,
    },
}

impl GrpcTlsConfig {
    #[must_use]
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    #[must_use]
    pub const fn mode_name(&self) -> &'static str {
        match self {
            Self::Disabled => "none",
            Self::Server { .. } => "server",
            Self::Mutual { .. } => "mutual",
        }
    }
}

impl Default for GrpcTlsConfig {
    fn default() -> Self {
        Self::disabled()
    }
}
