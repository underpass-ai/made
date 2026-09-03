use std::path::PathBuf;

use super::{
    MadeMcpGrpcTlsMode, GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV,
    GRPC_TLS_DOMAIN_NAME_ENV, GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV,
};

/// Configured gRPC TLS posture for the MCP client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MadeMcpGrpcTlsConfig {
    pub(crate) mode: MadeMcpGrpcTlsMode,
    pub(crate) ca_path: Option<PathBuf>,
    pub(crate) cert_path: Option<PathBuf>,
    pub(crate) key_path: Option<PathBuf>,
    pub(crate) domain_name: Option<String>,
}

impl MadeMcpGrpcTlsConfig {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            mode: MadeMcpGrpcTlsMode::Disabled,
            ca_path: None,
            cert_path: None,
            key_path: None,
            domain_name: None,
        }
    }

    #[must_use]
    pub fn server(ca_path: impl Into<PathBuf>, domain_name: Option<String>) -> Self {
        Self {
            mode: MadeMcpGrpcTlsMode::Server,
            ca_path: Some(ca_path.into()),
            cert_path: None,
            key_path: None,
            domain_name,
        }
    }

    #[must_use]
    pub fn mutual(
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
        domain_name: Option<String>,
    ) -> Self {
        Self {
            mode: MadeMcpGrpcTlsMode::Mutual,
            ca_path: Some(ca_path.into()),
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
            domain_name,
        }
    }

    #[must_use]
    pub fn from_env_for_endpoint(endpoint: Option<&str>) -> Self {
        let ca_path = optional_env_path(GRPC_TLS_CA_PATH_ENV);
        let cert_path = optional_env_path(GRPC_TLS_CERT_PATH_ENV);
        let key_path = optional_env_path(GRPC_TLS_KEY_PATH_ENV);
        let domain_name = optional_env_string(GRPC_TLS_DOMAIN_NAME_ENV);
        let server_tls_requested = ca_path.is_some()
            || domain_name.is_some()
            || endpoint.is_some_and(|endpoint| endpoint.trim().starts_with("https://"));
        let mode = optional_env_string(GRPC_TLS_MODE_ENV)
            .and_then(|value| parse_tls_mode(&value))
            .unwrap_or_else(|| {
                if cert_path.is_some() || key_path.is_some() {
                    MadeMcpGrpcTlsMode::Mutual
                } else if server_tls_requested {
                    MadeMcpGrpcTlsMode::Server
                } else {
                    MadeMcpGrpcTlsMode::Disabled
                }
            });

        Self {
            mode,
            ca_path,
            cert_path,
            key_path,
            domain_name,
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_for_endpoint(std::env::var(GRPC_ENDPOINT_ENV).ok().as_deref())
    }

    #[must_use]
    pub fn mode(&self) -> MadeMcpGrpcTlsMode {
        self.mode
    }

    #[must_use]
    pub fn mode_name(&self) -> &'static str {
        self.mode.as_str()
    }
}

fn parse_tls_mode(value: &str) -> Option<MadeMcpGrpcTlsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "disable" | "off" | "false" | "none" => Some(MadeMcpGrpcTlsMode::Disabled),
        "server" | "tls" => Some(MadeMcpGrpcTlsMode::Server),
        "mutual" | "mtls" | "m-tls" => Some(MadeMcpGrpcTlsMode::Mutual),
        _ => None,
    }
}

fn optional_env_path(name: &str) -> Option<PathBuf> {
    optional_env_string(name).map(PathBuf::from)
}

fn optional_env_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_mode_labels_are_stable() {
        assert_eq!(MadeMcpGrpcTlsMode::Disabled.as_str(), "disabled");
        assert_eq!(MadeMcpGrpcTlsMode::Server.as_str(), "server");
        assert_eq!(MadeMcpGrpcTlsMode::Mutual.as_str(), "mutual");
    }

    #[test]
    fn parse_tls_mode_accepts_aliases() {
        assert_eq!(parse_tls_mode("none"), Some(MadeMcpGrpcTlsMode::Disabled));
        assert_eq!(parse_tls_mode("tls"), Some(MadeMcpGrpcTlsMode::Server));
        assert_eq!(parse_tls_mode("mtls"), Some(MadeMcpGrpcTlsMode::Mutual));
        assert_eq!(parse_tls_mode("garbage"), None);
    }

    #[test]
    fn tls_constructors_preserve_paths() {
        let server = MadeMcpGrpcTlsConfig::server("/tmp/ca.pem", Some("made.local".into()));
        assert_eq!(server.mode(), MadeMcpGrpcTlsMode::Server);
        assert_eq!(
            server.ca_path.as_deref(),
            Some(std::path::Path::new("/tmp/ca.pem"))
        );
        assert_eq!(server.domain_name.as_deref(), Some("made.local"));

        let mutual =
            MadeMcpGrpcTlsConfig::mutual("/tmp/ca.pem", "/tmp/cert.pem", "/tmp/key.pem", None);
        assert_eq!(mutual.mode(), MadeMcpGrpcTlsMode::Mutual);
        assert_eq!(
            mutual.cert_path.as_deref(),
            Some(std::path::Path::new("/tmp/cert.pem"))
        );
        assert_eq!(
            mutual.key_path.as_deref(),
            Some(std::path::Path::new("/tmp/key.pem"))
        );
    }
}
