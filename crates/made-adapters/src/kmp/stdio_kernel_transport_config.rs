use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use made_core::error::DomainError;

const DEFAULT_BINARY: &str = "rehydration-mcp";
const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the kernel is and how patient to be with it.
#[derive(Debug, Clone)]
pub struct StdioKernelTransportConfig {
    binary: String,
    data_dir: PathBuf,
    call_timeout: Duration,
}

impl StdioKernelTransportConfig {
    /// A kernel keeping its memory in `data_dir`.
    ///
    /// The directory is the unit of exclusion: one kernel process per
    /// directory, so two hosts pointed at the same one is a
    /// configuration mistake the kernel will refuse rather than
    /// silently share.
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, DomainError> {
        let data_dir = data_dir.into();
        if data_dir.as_os_str().is_empty() {
            return Err(DomainError::EmptyField {
                field: "kmp.data_dir",
            });
        }
        Ok(Self {
            binary: DEFAULT_BINARY.to_owned(),
            data_dir,
            call_timeout: DEFAULT_CALL_TIMEOUT,
        })
    }

    #[must_use]
    pub fn with_binary(mut self, binary: impl Into<String>) -> Self {
        self.binary = binary.into();
        self
    }

    #[must_use]
    pub const fn with_call_timeout(mut self, call_timeout: Duration) -> Self {
        self.call_timeout = call_timeout;
        self
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    #[must_use]
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub(super) const fn call_timeout(&self) -> Duration {
        self.call_timeout
    }

    pub(super) fn environment(&self) -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("REHYDRATION_MCP_BACKEND", "embedded".to_owned()),
            (
                "REHYDRATION_MCP_DATA_DIR",
                self.data_dir.display().to_string(),
            ),
        ])
    }
}
