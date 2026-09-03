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

use made_core::error::DomainError;

/// TLS-mode override for the Runtime gRPC client.
pub const RUNTIME_TLS_MODE_ENV: &str = "MADE_RUNTIME_TLS_MODE";
/// CA bundle the client should trust when verifying the Runtime server.
pub const RUNTIME_TLS_CA_PATH_ENV: &str = "MADE_RUNTIME_TLS_CA_PATH";
/// Client certificate PEM (mutual TLS).
pub const RUNTIME_TLS_CERT_PATH_ENV: &str = "MADE_RUNTIME_TLS_CERT_PATH";
/// Client private key PEM (mutual TLS).
pub const RUNTIME_TLS_KEY_PATH_ENV: &str = "MADE_RUNTIME_TLS_KEY_PATH";
/// Optional TLS SNI/domain override when URL host differs from cert CN/SAN.
pub const RUNTIME_TLS_DOMAIN_NAME_ENV: &str = "MADE_RUNTIME_TLS_DOMAIN_NAME";

impl Default for RuntimeClientTlsConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

impl RuntimeClientTlsConfig {
    /// Explicitly disabled TLS — plain HTTP/2 over TCP.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            mode: RuntimeClientTlsMode::Disabled,
            ca_path: None,
            cert_path: None,
            key_path: None,
            domain_name: None,
        }
    }

    /// One-way TLS with a caller-supplied CA bundle.
    #[must_use]
    pub fn server(ca_path: impl Into<PathBuf>, domain_name: Option<String>) -> Self {
        Self {
            mode: RuntimeClientTlsMode::Server,
            ca_path: Some(ca_path.into()),
            cert_path: None,
            key_path: None,
            domain_name,
        }
    }

    /// Mutual TLS: client presents identity alongside verifying server.
    #[must_use]
    pub fn mutual(
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
        domain_name: Option<String>,
    ) -> Self {
        Self {
            mode: RuntimeClientTlsMode::Mutual,
            ca_path: Some(ca_path.into()),
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
            domain_name,
        }
    }

    /// Resolve the TLS posture from env with the same auto-detection
    /// the MCP adapter uses: `https://` endpoint OR `_CA_PATH` set OR
    /// `_DOMAIN_NAME` set → `Server`; presence of `_CERT_PATH` /
    /// `_KEY_PATH` → `Mutual`; explicit `_TLS_MODE` always wins.
    pub fn from_env_for_endpoint(endpoint: Option<&str>) -> Result<Self, DomainError> {
        let ca_path = optional_env_path(RUNTIME_TLS_CA_PATH_ENV);
        let cert_path = optional_env_path(RUNTIME_TLS_CERT_PATH_ENV);
        let key_path = optional_env_path(RUNTIME_TLS_KEY_PATH_ENV);
        let domain_name = optional_env_string(RUNTIME_TLS_DOMAIN_NAME_ENV);
        let server_tls_requested = ca_path.is_some()
            || domain_name.is_some()
            || endpoint.is_some_and(|endpoint| endpoint.trim().starts_with("https://"));

        let explicit_mode = optional_env_string(RUNTIME_TLS_MODE_ENV);
        let mode = match explicit_mode.as_deref() {
            Some(value) => parse_runtime_tls_mode(value).ok_or(DomainError::InvariantViolated {
                reason: "MADE_RUNTIME_TLS_MODE must be one of: disabled, server, mutual",
            })?,
            None => {
                if cert_path.is_some() || key_path.is_some() {
                    RuntimeClientTlsMode::Mutual
                } else if server_tls_requested {
                    RuntimeClientTlsMode::Server
                } else {
                    RuntimeClientTlsMode::Disabled
                }
            }
        };

        if mode == RuntimeClientTlsMode::Mutual && (cert_path.is_none() || key_path.is_none()) {
            return Err(DomainError::InvariantViolated {
                reason: "MADE_RUNTIME_TLS_MODE=mutual requires both _CERT_PATH and _KEY_PATH",
            });
        }

        Ok(Self {
            mode,
            ca_path,
            cert_path,
            key_path,
            domain_name,
        })
    }

    /// Stable label for startup logs and `Debug`-style output.
    #[must_use]
    pub fn mode_name(&self) -> &'static str {
        self.mode.as_str()
    }
}

fn parse_runtime_tls_mode(value: &str) -> Option<RuntimeClientTlsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "disable" | "off" | "false" | "none" => Some(RuntimeClientTlsMode::Disabled),
        "server" | "tls" => Some(RuntimeClientTlsMode::Server),
        "mutual" | "mtls" | "m-tls" => Some(RuntimeClientTlsMode::Mutual),
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

/// When TLS is enabled, rewrite an `http://` endpoint to `https://`
/// so callers can flip the TLS knob with a single env change.
pub(super) fn endpoint_uri_for_tls_mode(endpoint: &str, mode: RuntimeClientTlsMode) -> String {
    if mode == RuntimeClientTlsMode::Disabled {
        return endpoint.to_string();
    }
    endpoint.strip_prefix("http://").map_or_else(
        || endpoint.to_string(),
        |without_scheme| format!("https://{without_scheme}"),
    )
}
