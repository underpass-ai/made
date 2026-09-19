use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{LocalExecutionError, LocalExecutionMode, LocalNetworkPolicy};

/// Limits and host policy used by the trusted-local adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalExecutionConfig {
    pub(super) mode: LocalExecutionMode,
    pub(super) filesystem_root: PathBuf,
    pub(super) network: LocalNetworkPolicy,
    pub(super) timeout: Duration,
    pub(super) max_output_bytes: usize,
    pub(super) kill_process_group: bool,
}

impl LocalExecutionConfig {
    pub fn trusted_local(
        filesystem_root: impl AsRef<Path>,
        network: LocalNetworkPolicy,
        timeout: Duration,
        max_output_bytes: usize,
    ) -> Result<Self, LocalExecutionError> {
        Self::new(
            LocalExecutionMode::TrustedLocal,
            filesystem_root,
            network,
            timeout,
            max_output_bytes,
        )
    }

    pub fn new(
        mode: LocalExecutionMode,
        filesystem_root: impl AsRef<Path>,
        network: LocalNetworkPolicy,
        timeout: Duration,
        max_output_bytes: usize,
    ) -> Result<Self, LocalExecutionError> {
        if mode != LocalExecutionMode::TrustedLocal {
            return Err(LocalExecutionError::UnsupportedMode { mode });
        }
        if timeout.is_zero() {
            return Err(LocalExecutionError::InvalidTimeout);
        }
        if max_output_bytes == 0 {
            return Err(LocalExecutionError::InvalidOutputLimit);
        }
        let filesystem_root = filesystem_root
            .as_ref()
            .canonicalize()
            .map_err(|_| LocalExecutionError::FilesystemRootUnavailable)?;
        if !filesystem_root.is_dir() {
            return Err(LocalExecutionError::FilesystemRootNotDirectory);
        }
        Ok(Self {
            mode,
            filesystem_root,
            network,
            timeout,
            max_output_bytes,
            kill_process_group: true,
        })
    }

    #[must_use]
    pub const fn with_process_group_kill(mut self, enabled: bool) -> Self {
        self.kill_process_group = enabled;
        self
    }

    #[must_use]
    pub const fn mode(&self) -> LocalExecutionMode {
        self.mode
    }
    #[must_use]
    pub fn filesystem_root(&self) -> &Path {
        &self.filesystem_root
    }
    #[must_use]
    pub const fn network(&self) -> LocalNetworkPolicy {
        self.network
    }
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }
    #[must_use]
    pub const fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
    #[must_use]
    pub const fn process_group_kill_requested(&self) -> bool {
        self.kill_process_group
    }
}
