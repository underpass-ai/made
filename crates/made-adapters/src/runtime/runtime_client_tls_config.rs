use std::path::PathBuf;

use super::RuntimeClientTlsMode;

/// TLS posture for the outbound gRPC client to underpass-runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeClientTlsConfig {
    pub mode: RuntimeClientTlsMode,
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub domain_name: Option<String>,
}
