use made_core::DomainError;
use std::path::PathBuf;
use std::time::Duration;

/// Host-controlled OCI policy. Requests cannot override isolation or resource limits.
#[derive(Debug, Clone)]
pub struct OciExecutionConfig {
    pub image: String,
    pub workspace: PathBuf,
    pub operation_root: PathBuf,
    pub network: String,
    pub uid: u32,
    pub cpus: f64,
    pub memory_bytes: u64,
    pub pids: u32,
    pub max_output_bytes: usize,
    pub timeout: Duration,
}

impl OciExecutionConfig {
    pub(crate) fn validate(mut self) -> Result<Self, DomainError> {
        let valid_digest = self
            .image
            .rsplit_once("@sha256:")
            .is_some_and(|(name, digest)| {
                !name.is_empty()
                    && digest.len() == 64
                    && digest.bytes().all(|c| c.is_ascii_hexdigit())
            });
        if !valid_digest
            || self.uid == 0
            || !self.cpus.is_finite()
            || self.cpus <= 0.0
            || self.memory_bytes < 6 * 1024 * 1024
            || self.pids == 0
            || self.max_output_bytes == 0
            || self.timeout.is_zero()
            || self.network.is_empty()
            || self.network == "host"
            || self.network.starts_with("container:")
        {
            return Err(super::operation_record::invalid(
                "invalid OCI execution policy",
            ));
        }
        self.workspace = self
            .workspace
            .canonicalize()
            .map_err(|_| super::operation_record::invalid("OCI workspace unavailable"))?;
        if !self.workspace.is_dir() || self.workspace.to_string_lossy().contains(',') {
            return Err(super::operation_record::invalid("invalid OCI workspace"));
        }
        std::fs::create_dir_all(&self.operation_root)
            .map_err(|_| super::operation_record::invalid("OCI operation root unavailable"))?;
        self.operation_root = self
            .operation_root
            .canonicalize()
            .map_err(|_| super::operation_record::invalid("OCI operation root unavailable"))?;
        if self.operation_root.starts_with(&self.workspace) {
            return Err(super::operation_record::invalid(
                "OCI operation evidence must be outside workspace",
            ));
        }
        Ok(self)
    }
}
