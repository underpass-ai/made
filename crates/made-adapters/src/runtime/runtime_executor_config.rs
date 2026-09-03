use super::{RuntimeClientTlsConfig, RuntimePrincipal};

/// Configuration for the Runtime executor adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutorConfig {
    pub grpc_endpoint: String,
    pub principal: RuntimePrincipal,
    pub tls: RuntimeClientTlsConfig,
}

use made_core::error::DomainError;

const DEFAULT_RUNTIME_GRPC_ENDPOINT: &str = "http://underpass-runtime:50053";
const DEFAULT_RUNTIME_TENANT_ID: &str = "made";
const DEFAULT_RUNTIME_ACTOR_ID: &str = "made";
const DEFAULT_RUNTIME_ROLE: &str = "developer";

impl RuntimeExecutorConfig {
    pub fn from_env() -> Result<Self, DomainError> {
        let grpc_endpoint =
            env_or_default("MADE_RUNTIME_GRPC_ENDPOINT", DEFAULT_RUNTIME_GRPC_ENDPOINT)?;
        let tenant_id = env_or_default(
            "MADE_RUNTIME_PRINCIPAL_TENANT_ID",
            DEFAULT_RUNTIME_TENANT_ID,
        )?;
        let actor_id = env_or_default("MADE_RUNTIME_PRINCIPAL_ACTOR_ID", DEFAULT_RUNTIME_ACTOR_ID)?;
        let roles = parse_roles(std::env::var("MADE_RUNTIME_PRINCIPAL_ROLES").ok())?;
        let tls = RuntimeClientTlsConfig::from_env_for_endpoint(Some(&grpc_endpoint))?;

        Ok(Self {
            grpc_endpoint,
            principal: RuntimePrincipal {
                tenant_id,
                actor_id,
                roles,
            },
            tls,
        })
    }
}

fn env_or_default(key: &str, default: &str) -> Result<String, DomainError> {
    let value = std::env::var(key).unwrap_or_else(|_| default.to_owned());
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField {
            field: "runtime.config",
        });
    }
    Ok(trimmed.to_owned())
}

fn parse_roles(raw: Option<String>) -> Result<Vec<String>, DomainError> {
    let roles = raw
        .unwrap_or_else(|| DEFAULT_RUNTIME_ROLE.to_owned())
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if roles.is_empty() {
        return Err(DomainError::EmptyCollection {
            field: "runtime.principal.roles",
        });
    }
    Ok(roles)
}
