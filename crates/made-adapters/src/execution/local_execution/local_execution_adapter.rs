use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::{
    LocalExecutionConfig, LocalExecutionError, LocalExecutionMode, LocalExecutionOutcome,
    LocalExecutionPort, LocalExecutionReceipt, LocalExecutionRequest, LocalProcessTermination,
};

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
        let stdout_task = tokio::spawn(capture_output(
            stdout,
            Arc::clone(&output_bytes),
            self.config.max_output_bytes,
            event_sender.clone(),
        ));
        let stderr_task = tokio::spawn(capture_output(
            stderr,
            Arc::clone(&output_bytes),
            self.config.max_output_bytes,
            event_sender,
        ));

        let (outcome, status, termination) = await_process(
            &mut child,
            &mut event_receiver,
            self.config.timeout,
            self.config.kill_process_group,
        )
        .await?;

        let stdout_result = stdout_task
            .await
            .map_err(|_| LocalExecutionError::OutputCapture)?;
        let stderr_result = stderr_task
            .await
            .map_err(|_| LocalExecutionError::OutputCapture)?;
        if outcome.is_none() && (stdout_result.is_err() || stderr_result.is_err()) {
            return Err(LocalExecutionError::OutputCapture);
        }
        let (stdout_bytes, stdout_exceeded) = stdout_result.unwrap_or((0, false));
        let (stderr_bytes, stderr_exceeded) = stderr_result.unwrap_or((0, false));
        let outcome = outcome.or_else(|| {
            (stdout_exceeded || stderr_exceeded)
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
            stdout_bytes: stdout_bytes as u64,
            stderr_bytes: stderr_bytes as u64,
            output_bytes: output_bytes.load(Ordering::Relaxed) as u64,
            process_termination: termination,
        })
    }
}

async fn await_process(
    child: &mut Child,
    event_receiver: &mut mpsc::UnboundedReceiver<Result<(), ()>>,
    timeout: std::time::Duration,
    kill_process_group: bool,
) -> Result<
    (
        Option<LocalExecutionOutcome>,
        Option<ExitStatus>,
        LocalProcessTermination,
    ),
    LocalExecutionError,
> {
    enum ProcessEnd {
        Exited(Result<ExitStatus, LocalExecutionError>),
        OutputLimitExceeded,
        CaptureFailed,
    }

    let process_end = tokio::time::timeout(timeout, async {
        tokio::select! {
            status = child.wait() => ProcessEnd::Exited(status.map_err(|_| LocalExecutionError::Wait)),
            event = event_receiver.recv() => match event {
                Some(Ok(())) => ProcessEnd::OutputLimitExceeded,
                Some(Err(())) | None => ProcessEnd::CaptureFailed,
            }
        }
    })
    .await;
    match process_end {
        Ok(ProcessEnd::Exited(status)) => Ok((
            None,
            Some(status?),
            LocalProcessTermination::not_requested(),
        )),
        Ok(ProcessEnd::OutputLimitExceeded) => {
            let termination = terminate_child(child, kill_process_group);
            let status = child.wait().await.map_err(|_| LocalExecutionError::Wait)?;
            Ok((
                Some(LocalExecutionOutcome::OutputLimitExceeded),
                Some(status),
                termination,
            ))
        }
        Ok(ProcessEnd::CaptureFailed) => {
            let _ = terminate_child(child, kill_process_group);
            let _ = child.wait().await;
            Err(LocalExecutionError::OutputCapture)
        }
        Err(_) => {
            let termination = terminate_child(child, kill_process_group);
            let status = child.wait().await.map_err(|_| LocalExecutionError::Wait)?;
            Ok((
                Some(LocalExecutionOutcome::TimedOut),
                Some(status),
                termination,
            ))
        }
    }
}

#[async_trait]
impl LocalExecutionPort for LocalExecutionAdapter {
    async fn execute(
        &self,
        request: LocalExecutionRequest,
    ) -> Result<LocalExecutionReceipt, LocalExecutionError> {
        self.execute_request(request).await
    }
}

async fn capture_output<R: AsyncRead + Unpin>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    max_output_bytes: usize,
    event_sender: mpsc::UnboundedSender<Result<(), ()>>,
) -> Result<(usize, bool), ()> {
    let mut buffer = [0_u8; 8192];
    let mut captured = 0_usize;
    loop {
        let Ok(read) = reader.read(&mut buffer).await else {
            let _ = event_sender.send(Err(()));
            return Err(());
        };
        if read == 0 {
            return Ok((captured, false));
        }
        captured = captured.saturating_add(read);
        let observed = total
            .fetch_add(read, Ordering::Relaxed)
            .saturating_add(read);
        if observed > max_output_bytes {
            let _ = event_sender.send(Ok(()));
            return Ok((captured, true));
        }
    }
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
    let group_killed = group_requested && kill_process_group(pid);
    #[cfg(not(unix))]
    let group_killed = false;
    let direct_killed = group_killed || child.start_kill().is_ok();
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
        .arg("--")
        .arg(format!("-{pid}"))
        .status()
        .is_ok_and(|status| status.success())
}
