use super::RuntimeExecutorConfig;

/// Binary-level execution backend selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutorBackendConfig {
    Noop,
    Runtime(RuntimeExecutorConfig),
}

use made_core::error::DomainError;

impl ExecutorBackendConfig {
    /// Load executor selection from `MADE_*` environment variables.
    ///
    /// Supported variables:
    ///
    /// - `MADE_EXECUTOR_KIND`: `noop` (default) or `runtime`
    /// - `MADE_RUNTIME_GRPC_ENDPOINT`
    /// - `MADE_RUNTIME_PRINCIPAL_TENANT_ID`
    /// - `MADE_RUNTIME_PRINCIPAL_ACTOR_ID`
    /// - `MADE_RUNTIME_PRINCIPAL_ROLES` (comma-separated)
    pub fn from_env() -> Result<Self, DomainError> {
        let kind = std::env::var("MADE_EXECUTOR_KIND")
            .unwrap_or_else(|_| "noop".to_owned())
            .trim()
            .to_ascii_lowercase();

        match kind.as_str() {
            "" | "noop" => Ok(Self::Noop),
            "runtime" => Ok(Self::Runtime(RuntimeExecutorConfig::from_env()?)),
            _ => Err(DomainError::InvariantViolated {
                reason: "invalid executor backend kind",
            }),
        }
    }
}
