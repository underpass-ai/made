#![allow(
    clippy::format_push_string,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::never_loop,
    clippy::single_match_else,
    clippy::struct_excessive_bools,
    clippy::too_many_lines
)]

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Runtime mode declared by the local execution adapter.
///
/// `TrustedLocal` is deliberately not a sandbox claim: the host process and
/// its operating-system policy remain authoritative. The isolated Linux mode
/// is represented so callers can report its absence without silently falling
/// back to a stronger-looking mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalExecutionMode {
    #[serde(rename = "trusted-local")]
    TrustedLocal,
    #[serde(rename = "isolated-linux-unavailable")]
    IsolatedLinuxUnavailable,
}

/// Network permission declared for a trusted-local invocation.
///
/// This is an explicit declaration in the receipt, not an OS-level network
/// namespace or firewall enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalNetworkPolicy {
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "not-allowed")]
    NotAllowed,
}

/// Limits and host policy used by [`LocalExecutionAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalExecutionConfig {
    mode: LocalExecutionMode,
    filesystem_root: PathBuf,
    network: LocalNetworkPolicy,
    timeout: Duration,
    max_output_bytes: usize,
    kill_process_group: bool,
}

impl LocalExecutionConfig {
    /// Build a trusted-local configuration from an existing directory.
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

    /// Build a configuration while rejecting modes not available in this
    /// adapter. The rejected mode remains representable in the public enum so
    /// a host can make unavailability observable instead of claiming isolation.
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

    /// Whether the adapter should place the child in a dedicated process
    /// group and terminate that group on timeout or output overflow.
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

/// Direct-exec request. The adapter never invokes a shell and never inherits
/// the caller's environment; only the explicitly supplied environment is
/// passed to the child.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalExecutionRequest {
    executable: PathBuf,
    args: Vec<String>,
    environment: BTreeMap<String, String>,
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

    fn request_digest(&self) -> String {
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
        hex_digest(&digest.finalize())
    }
}

/// Why a local process stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalExecutionOutcome {
    Completed,
    Failed,
    TimedOut,
    OutputLimitExceeded,
}

/// Structured process-group termination evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProcessTermination {
    pub requested: bool,
    pub supported: bool,
    pub attempted: bool,
    pub succeeded: bool,
}

/// Receipt for one local invocation.
///
/// It intentionally contains counts and a one-way request digest, never
/// command arguments, environment values, stdout, stderr, or error text from
/// the child. The mode and network fields are declarations of the host policy;
/// this adapter does not claim strong isolation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalExecutionReceipt {
    pub mode: LocalExecutionMode,
    pub filesystem_root: PathBuf,
    pub network: LocalNetworkPolicy,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    pub request_digest: String,
    pub outcome: LocalExecutionOutcome,
    pub exit_code: Option<i32>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub output_bytes: u64,
    pub process_termination: LocalProcessTermination,
}

/// Port exposed by the local execution boundary.
#[async_trait]
pub trait LocalExecutionPort: Send + Sync {
    async fn execute(
        &self,
        request: LocalExecutionRequest,
    ) -> Result<LocalExecutionReceipt, LocalExecutionError>;
}

/// Trusted-local direct process adapter with explicit resource boundaries.
#[derive(Debug, Clone)]
pub struct LocalExecutionAdapter {
    config: LocalExecutionConfig,
}

impl LocalExecutionAdapter {
    pub fn new(config: LocalExecutionConfig) -> Result<Self, LocalExecutionError> {
        if config.mode != LocalExecutionMode::TrustedLocal {
            return Err(LocalExecutionError::UnsupportedMode { mode: config.mode });
        }
        Ok(Self { config })
    }

    #[must_use]
    pub const fn config(&self) -> &LocalExecutionConfig {
        &self.config
    }

    fn resolve_executable(
        &self,
        request: &LocalExecutionRequest,
    ) -> Result<PathBuf, LocalExecutionError> {
        let candidate = if request.executable.is_absolute() {
            request.executable.clone()
        } else {
            self.config.filesystem_root.join(&request.executable)
        };
        let executable = candidate
            .canonicalize()
            .map_err(|_| LocalExecutionError::ExecutableUnavailable)?;
        if !executable.starts_with(&self.config.filesystem_root) {
            return Err(LocalExecutionError::ExecutableOutsideRoot);
        }
        if !executable.is_file() {
            return Err(LocalExecutionError::ExecutableNotFile);
        }
        Ok(executable)
    }

    async fn execute_request(
        &self,
        request: LocalExecutionRequest,
    ) -> Result<LocalExecutionReceipt, LocalExecutionError> {
        let executable = self.resolve_executable(&request)?;
        let request_digest = request.request_digest();
        let output_bytes = Arc::new(AtomicUsize::new(0));
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();

        let mut command = Command::new(&executable);
        command
            .current_dir(&self.config.filesystem_root)
            .env_clear()
            .args(&request.args)
            .envs(&request.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure_process_group(&mut command, self.config.kill_process_group);

        let mut child = command
            .spawn()
            .map_err(|source| LocalExecutionError::Spawn { source })?;
        let stdout = child
            .stdout
            .take()
            .ok_or(LocalExecutionError::OutputPipeUnavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(LocalExecutionError::OutputPipeUnavailable)?;

        let stdout_bytes = Arc::clone(&output_bytes);
        let stdout_sender = event_sender.clone();
        let stdout_task = tokio::spawn(capture_output(
            stdout,
            stdout_bytes,
            self.config.max_output_bytes,
            stdout_sender,
        ));
        let stderr_bytes = Arc::clone(&output_bytes);
        let stderr_task = tokio::spawn(capture_output(
            stderr,
            stderr_bytes,
            self.config.max_output_bytes,
            event_sender,
        ));

        let process_end = tokio::time::timeout(self.config.timeout, async {
            loop {
                tokio::select! {
                    status = child.wait() => {
                        return ProcessEnd::Exited(status.map_err(|_| LocalExecutionError::Wait));
                    }
                    event = event_receiver.recv() => match event {
                        Some(CaptureEvent::OutputLimitExceeded) => return ProcessEnd::OutputLimitExceeded,
                        Some(CaptureEvent::CaptureFailed) => return ProcessEnd::CaptureFailed,
                        None => return ProcessEnd::CaptureFailed,
                    }
                }
            }
        })
        .await;

        let (outcome, status, termination) = match process_end {
            Ok(ProcessEnd::Exited(status)) => {
                let status = status?;
                (None, Some(status), LocalProcessTermination::not_requested())
            }
            Ok(ProcessEnd::OutputLimitExceeded) => {
                let termination = terminate_child(&mut child, self.config.kill_process_group);
                let status = child.wait().await.map_err(|_| LocalExecutionError::Wait)?;
                (
                    Some(LocalExecutionOutcome::OutputLimitExceeded),
                    Some(status),
                    termination,
                )
            }
            Ok(ProcessEnd::CaptureFailed) => {
                let _ = terminate_child(&mut child, self.config.kill_process_group);
                let _ = child.wait().await;
                return Err(LocalExecutionError::OutputCapture);
            }
            Err(_) => {
                let termination = terminate_child(&mut child, self.config.kill_process_group);
                let status = child.wait().await.map_err(|_| LocalExecutionError::Wait)?;
                (
                    Some(LocalExecutionOutcome::TimedOut),
                    Some(status),
                    termination,
                )
            }
        };

        let stdout_result = stdout_task
            .await
            .map_err(|_| LocalExecutionError::OutputCapture)?;
        let stderr_result = stderr_task
            .await
            .map_err(|_| LocalExecutionError::OutputCapture)?;
        if outcome.is_none() && (stdout_result.is_err() || stderr_result.is_err()) {
            return Err(LocalExecutionError::OutputCapture);
        }

        let stdout_result = stdout_result.unwrap_or(CaptureResult {
            bytes: 0,
            exceeded: false,
        });
        let stderr_result = stderr_result.unwrap_or(CaptureResult {
            bytes: 0,
            exceeded: false,
        });
        let outcome = outcome.or_else(|| {
            (stdout_result.exceeded || stderr_result.exceeded)
                .then_some(LocalExecutionOutcome::OutputLimitExceeded)
        });
        let outcome = outcome.unwrap_or_else(|| {
            if status.as_ref().is_some_and(ExitStatus::success) {
                LocalExecutionOutcome::Completed
            } else {
                LocalExecutionOutcome::Failed
            }
        });

        Ok(LocalExecutionReceipt {
            mode: self.config.mode,
            filesystem_root: self.config.filesystem_root.clone(),
            network: self.config.network,
            timeout_ms: self
                .config
                .timeout
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            max_output_bytes: self.config.max_output_bytes as u64,
            request_digest,
            outcome,
            exit_code: status.and_then(|value| value.code()),
            stdout_bytes: stdout_result.bytes as u64,
            stderr_bytes: stderr_result.bytes as u64,
            output_bytes: output_bytes.load(Ordering::Relaxed) as u64,
            process_termination: termination,
        })
    }
}

/// Name used by callers that treat the adapter as a concrete connector.
pub type LocalExecutionConnector = LocalExecutionAdapter;

/// Name used by callers that want to refer to the bounded policy as limits.
pub type LocalExecutionLimits = LocalExecutionConfig;

#[async_trait]
impl LocalExecutionPort for LocalExecutionAdapter {
    async fn execute(
        &self,
        request: LocalExecutionRequest,
    ) -> Result<LocalExecutionReceipt, LocalExecutionError> {
        self.execute_request(request).await
    }
}

#[derive(Debug)]
enum ProcessEnd {
    Exited(Result<ExitStatus, LocalExecutionError>),
    OutputLimitExceeded,
    CaptureFailed,
}

#[derive(Debug, Clone, Copy)]
enum CaptureEvent {
    OutputLimitExceeded,
    CaptureFailed,
}

async fn capture_output<R: AsyncRead + Unpin>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    max_output_bytes: usize,
    event_sender: mpsc::UnboundedSender<CaptureEvent>,
) -> Result<CaptureResult, ()> {
    let mut buffer = [0_u8; 8192];
    let mut captured = 0_usize;
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            Err(_) => {
                let _ = event_sender.send(CaptureEvent::CaptureFailed);
                return Err(());
            }
        };
        if read == 0 {
            return Ok(CaptureResult {
                bytes: captured,
                exceeded: false,
            });
        }
        captured = captured.saturating_add(read);
        let observed = total
            .fetch_add(read, Ordering::Relaxed)
            .saturating_add(read);
        if observed > max_output_bytes {
            let _ = event_sender.send(CaptureEvent::OutputLimitExceeded);
            return Ok(CaptureResult {
                bytes: captured,
                exceeded: true,
            });
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CaptureResult {
    bytes: usize,
    exceeded: bool,
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command, enabled: bool) {
    if enabled {
        command.process_group(0);
    }
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command, _enabled: bool) {}

fn terminate_child(child: &mut Child, group_requested: bool) -> LocalProcessTermination {
    let Some(pid) = child.id() else {
        return LocalProcessTermination {
            requested: group_requested,
            supported: false,
            attempted: false,
            succeeded: false,
        };
    };

    #[cfg(unix)]
    let group_killed = if group_requested {
        kill_process_group(pid)
    } else {
        false
    };
    #[cfg(not(unix))]
    let group_killed = false;

    let direct_killed = if group_killed {
        true
    } else {
        child.start_kill().is_ok()
    };
    LocalProcessTermination {
        requested: group_requested,
        supported: cfg!(unix),
        attempted: true,
        succeeded: direct_killed,
    }
}

#[cfg(unix)]
fn kill_process_group(pid: u32) -> bool {
    let Some(kill_binary) = ["/bin/kill", "/usr/bin/kill"]
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
    else {
        return false;
    };
    std::process::Command::new(kill_binary)
        .arg("-KILL")
        .arg(format!("-{pid}"))
        .status()
        .is_ok_and(|status| status.success())
}

#[derive(Debug, Error)]
pub enum LocalExecutionError {
    #[error("local execution mode {mode:?} is unsupported: isolated Linux runtime is unavailable")]
    UnsupportedMode { mode: LocalExecutionMode },
    #[error("local execution filesystem root is unavailable")]
    FilesystemRootUnavailable,
    #[error("local execution filesystem root must be a directory")]
    FilesystemRootNotDirectory,
    #[error("local execution timeout must be non-zero")]
    InvalidTimeout,
    #[error("local execution output limit must be non-zero")]
    InvalidOutputLimit,
    #[error("local execution executable is unavailable")]
    ExecutableUnavailable,
    #[error("local execution executable must remain below the configured filesystem root")]
    ExecutableOutsideRoot,
    #[error("local execution executable is not a file")]
    ExecutableNotFile,
    #[error("local execution output pipes are unavailable")]
    OutputPipeUnavailable,
    #[error("local execution process could not be spawned")]
    Spawn { source: io::Error },
    #[error("local execution process could not be awaited")]
    Wait,
    #[error("local execution output capture failed")]
    OutputCapture,
}

impl LocalProcessTermination {
    const fn not_requested() -> Self {
        Self {
            requested: false,
            supported: cfg!(unix),
            attempted: false,
            succeeded: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    fn script(root: &TempDir, body: &str) -> PathBuf {
        let path = root.path().join("run.sh");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn adapter(root: &TempDir, timeout: Duration, output_limit: usize) -> LocalExecutionAdapter {
        LocalExecutionAdapter::new(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::NotAllowed,
                timeout,
                output_limit,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_executable_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let executable = script(&outside, "exit 0");
        let error = adapter(&root, Duration::from_secs(1), 1024)
            .execute(LocalExecutionRequest::new(executable))
            .await
            .unwrap_err();
        assert!(matches!(error, LocalExecutionError::ExecutableOutsideRoot));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_returns_structured_receipt_and_terminates_group() {
        let root = tempfile::tempdir().unwrap();
        let executable = script(&root, "sleep 5");
        let receipt = adapter(&root, Duration::from_millis(50), 1024)
            .execute(LocalExecutionRequest::new(executable))
            .await
            .unwrap();
        assert_eq!(receipt.outcome, LocalExecutionOutcome::TimedOut);
        assert!(receipt.process_termination.requested);
        assert!(receipt.process_termination.attempted);
        assert!(receipt.process_termination.succeeded);
        assert_eq!(receipt.network, LocalNetworkPolicy::NotAllowed);
        assert_eq!(receipt.mode, LocalExecutionMode::TrustedLocal);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_limit_returns_counts_without_output_content() {
        let root = tempfile::tempdir().unwrap();
        let executable = script(&root, "printf 'super-secret-output-123456789'; sleep 5");
        let receipt = adapter(&root, Duration::from_secs(1), 4)
            .execute(
                LocalExecutionRequest::new(executable)
                    .with_args(["secret-argument"])
                    .with_environment("TOKEN", "secret-environment-value"),
            )
            .await
            .unwrap();
        assert_eq!(receipt.outcome, LocalExecutionOutcome::OutputLimitExceeded);
        assert!(receipt.output_bytes > receipt.max_output_bytes);
        assert!(receipt.process_termination.succeeded);
        let serialized = serde_json::to_string(&receipt).unwrap();
        assert!(!serialized.contains("super-secret-output"));
        assert!(!serialized.contains("secret-argument"));
        assert!(!serialized.contains("secret-environment-value"));
    }

    #[test]
    fn unsupported_isolated_mode_is_explicitly_rejected() {
        let root = tempfile::tempdir().unwrap();
        let error = LocalExecutionConfig::new(
            LocalExecutionMode::IsolatedLinuxUnavailable,
            root.path(),
            LocalNetworkPolicy::Allowed,
            Duration::from_secs(1),
            1024,
        )
        .unwrap_err();
        assert!(matches!(error, LocalExecutionError::UnsupportedMode { .. }));
        assert_eq!(
            serde_json::to_string(&LocalExecutionMode::IsolatedLinuxUnavailable).unwrap(),
            "\"isolated-linux-unavailable\""
        );
    }

    #[test]
    fn invalid_limits_and_root_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::Allowed,
                Duration::ZERO,
                1,
            ),
            Err(LocalExecutionError::InvalidTimeout)
        ));
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::Allowed,
                Duration::from_secs(1),
                0,
            ),
            Err(LocalExecutionError::InvalidOutputLimit)
        ));
        let file = root.path().join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                &file,
                LocalNetworkPolicy::Allowed,
                Duration::from_secs(1),
                1,
            ),
            Err(LocalExecutionError::FilesystemRootNotDirectory)
        ));
    }
}
