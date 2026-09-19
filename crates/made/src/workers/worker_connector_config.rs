use std::path::PathBuf;
use std::time::Duration;

use made_adapters::execution::OciExecutionConfig;
use made_core::DomainError;

use super::git_worker_connector_config::GitWorkerConnectorConfig;
use super::http_worker_connector_config::HttpWorkerConnectorConfig;
use super::worker_environment::{env_parse, invalid, optional, required};

/// Deployment configuration for exactly one installed connector.
#[derive(Debug, Clone)]
pub(crate) enum WorkerConnectorConfig {
    Oci(OciExecutionConfig),
    Git(GitWorkerConnectorConfig),
    Http(HttpWorkerConnectorConfig),
}

impl WorkerConnectorConfig {
    pub(crate) fn from_env() -> Result<Self, DomainError> {
        match required("MADE_WORKER_CONNECTOR")?.as_str() {
            "oci" => Ok(Self::Oci(OciExecutionConfig {
                image: required("MADE_WORKER_OCI_IMAGE")?,
                workspace: PathBuf::from(required("MADE_WORKER_OCI_WORKSPACE")?),
                operation_root: PathBuf::from(required("MADE_WORKER_OPERATION_ROOT")?),
                network: optional("MADE_WORKER_OCI_NETWORK").unwrap_or_else(|| "none".to_owned()),
                uid: env_parse("MADE_WORKER_OCI_UID", 65_532_u32)?,
                cpus: env_parse("MADE_WORKER_OCI_CPUS", 1.0_f64)?,
                memory_bytes: env_parse("MADE_WORKER_OCI_MEMORY_BYTES", 536_870_912_u64)?,
                pids: env_parse("MADE_WORKER_OCI_PIDS", 128_u32)?,
                max_output_bytes: env_parse("MADE_WORKER_OCI_MAX_OUTPUT_BYTES", 1_048_576_usize)?,
                timeout: timeout()?,
            })),
            "git" => Ok(Self::Git(GitWorkerConnectorConfig {
                repository: PathBuf::from(required("MADE_WORKER_GIT_REPOSITORY")?),
                scratch: PathBuf::from(required("MADE_WORKER_GIT_SCRATCH")?),
            })),
            "http" => Ok(Self::Http(HttpWorkerConnectorConfig {
                base_url: required("MADE_WORKER_HTTP_BASE")?,
                operation_root: PathBuf::from(required("MADE_WORKER_OPERATION_ROOT")?),
                timeout: timeout()?,
            })),
            _ => Err(invalid("MADE_WORKER_CONNECTOR must be oci, git, or http")),
        }
    }

    pub(crate) const fn id(&self) -> &'static str {
        match self {
            Self::Oci(_) => "oci",
            Self::Git(_) => "git",
            Self::Http(_) => "http",
        }
    }
}

fn timeout() -> Result<Duration, DomainError> {
    Ok(Duration::from_millis(env_parse(
        "MADE_WORKER_CONNECTOR_TIMEOUT_MS",
        300_000_u64,
    )?))
}
