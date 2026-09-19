use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Direct-exec request with explicit arguments and environment.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalExecutionRequest {
    pub(super) executable: PathBuf,
    pub(super) args: Vec<String>,
    pub(super) environment: BTreeMap<String, String>,
}

impl fmt::Debug for LocalExecutionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalExecutionRequest")
            .field("executable", &self.executable)
            .field("argument_count", &self.args.len())
            .field(
                "environment_keys",
                &self.environment.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl LocalExecutionRequest {
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            environment: BTreeMap::new(),
        }
    }
    #[must_use]
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }
    #[must_use]
    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub(super) fn request_digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"made.local-execution.v1\0");
        digest.update(self.executable.to_string_lossy().as_bytes());
        digest.update([0]);
        for arg in &self.args {
            digest.update(arg.as_bytes());
            digest.update([0]);
        }
        for (key, value) in &self.environment {
            digest.update(key.as_bytes());
            digest.update([0]);
            digest.update(value.as_bytes());
            digest.update([0]);
        }
        format!("{:x}", digest.finalize())
    }
}
