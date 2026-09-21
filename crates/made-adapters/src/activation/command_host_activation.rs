use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{HostActivationOutcome, HostActivationPort};
use made_core::value_objects::{
    DeliveryFailureReason, HostActivationAdapterKind, HostActivationEnvelope,
    HostActivationReceipt, HostDeliveryRecord, HostTransportRef, IntegratorBinding,
};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

use super::{HostActivationCommand, HostActivationConfigError};

/// Environment variable naming the one command this adapter runs.
pub const COMMAND_ENV: &str = "MADE_HOST_ACTIVATION_COMMAND";
/// How long the command may take before the wake-up counts as failed.
pub const TIMEOUT_MS_ENV: &str = "MADE_HOST_ACTIVATION_TIMEOUT_MS";
/// How much of the command's output is kept, in bytes.
pub const MAX_OUTPUT_ENV: &str = "MADE_HOST_ACTIVATION_MAX_OUTPUT";

/// The destination the envelope is addressed to, in the child's environment.
pub const DESTINATION_VAR: &str = "MADE_ACTIVATION_DESTINATION";
/// What kind of host that destination is.
pub const HOST_KIND_VAR: &str = "MADE_ACTIVATION_HOST_KIND";
/// Which delivery is being handed over.
pub const DELIVERY_ID_VAR: &str = "MADE_ACTIVATION_DELIVERY_ID";

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_MAX_OUTPUT: u64 = 65_536;
const TRANSPORT_REF_BYTES: usize = 256;

/// Wake a host by running the operator's own command.
///
/// The command is configuration, resolved once at startup; the envelope
/// is input, arriving on the child's standard input as JSON. Nothing in
/// the envelope selects a program, an argument or a shell, which is the
/// whole difference between waking a host and letting a payload choose
/// what runs. The child is started with a cleared environment plus three
/// named variables, so what it can reach is what its operator gave it
/// and not whatever the engine happened to be started with.
///
/// Exit zero is transport and nothing more: the host was handed the
/// envelope. Whether it read it, acted on it or acknowledged it is a
/// separate question the loop answers through its own commands.
#[derive(Debug, Clone)]
pub struct CommandHostActivation {
    command: HostActivationCommand,
    timeout: Duration,
    max_output: usize,
}

impl CommandHostActivation {
    /// Build the adapter over an already resolved command.
    #[must_use]
    pub const fn new(command: HostActivationCommand, timeout: Duration, max_output: usize) -> Self {
        Self {
            command,
            timeout,
            max_output,
        }
    }

    /// Read the adapter an operator configured, if they configured one.
    ///
    /// `Ok(None)` is the deployment that does not wake hosts: the
    /// absence of a command is a normal, declared state rather than a
    /// failure. A command that cannot be resolved is a failure, and one
    /// worth refusing to start over.
    ///
    /// # Errors
    ///
    /// Returns the configuration failure when the command names nothing
    /// runnable, or when a bound is not a positive whole number.
    pub fn from_env() -> Result<Option<Self>, HostActivationConfigError> {
        let Some(raw) = read_env(COMMAND_ENV) else {
            return Ok(None);
        };
        Ok(Some(Self {
            command: HostActivationCommand::parse(&raw, COMMAND_ENV)?,
            timeout: Duration::from_millis(bound(TIMEOUT_MS_ENV, DEFAULT_TIMEOUT_MS)?),
            max_output: usize::try_from(bound(MAX_OUTPUT_ENV, DEFAULT_MAX_OUTPUT)?)
                .unwrap_or(usize::MAX),
        }))
    }

    /// The command this adapter runs, for an operator reading its configuration.
    #[must_use]
    pub const fn command(&self) -> &HostActivationCommand {
        &self.command
    }

    async fn run(
        &self,
        binding: &IntegratorBinding,
        record: &HostDeliveryRecord,
        envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, DomainError> {
        let payload =
            serde_json::to_vec(envelope).map_err(|error| DomainError::InvalidDocument {
                reason: format!("the activation envelope did not serialise: {error}"),
            })?;
        let destination = binding.destination();
        let mut command = Command::new(self.command.executable());
        command
            .args(self.command.args())
            .env_clear()
            .env(DESTINATION_VAR, destination.address().as_str())
            .env(HOST_KIND_VAR, destination.host_kind().as_str())
            .env(DELIVERY_ID_VAR, record.id().as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return failed(&format!("the activation command did not start: {error}")),
        };
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let writer = tokio::spawn(async move {
            if let Some(mut stdin) = stdin {
                let _ = stdin.write_all(&payload).await;
                let _ = stdin.shutdown().await;
            }
        });
        let limit = self.max_output;
        let out = tokio::spawn(async move { capture(stdout, limit).await });
        let err = tokio::spawn(async move { capture(stderr, limit).await });
        let status = match timeout(self.timeout, child.wait()).await {
            Err(_) => {
                let _ = child.start_kill();
                writer.abort();
                out.abort();
                err.abort();
                return failed(&format!(
                    "the activation command did not answer within {}ms",
                    self.timeout.as_millis()
                ));
            }
            Ok(Err(error)) => {
                return failed(&format!(
                    "the activation command could not be waited on: {error}"
                ))
            }
            Ok(Ok(status)) => status,
        };
        let _ = writer.await;
        let stdout = out.await.unwrap_or_default();
        if status.success() {
            return Ok(HostActivationOutcome::Accepted(HostActivationReceipt::new(
                HostActivationAdapterKind::Command,
                OffsetDateTime::now_utc(),
                transport_ref(&stdout),
            )));
        }
        let stderr = err.await.unwrap_or_default();
        failed(&format!(
            "the activation command exited {}: {}",
            status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
            summary(&stderr).unwrap_or_else(|| "no output".to_owned())
        ))
    }
}

#[async_trait]
impl HostActivationPort for CommandHostActivation {
    async fn activate(
        &self,
        binding: &IntegratorBinding,
        record: &HostDeliveryRecord,
        envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, DomainError> {
        self.run(binding, record, envelope).await
    }

    fn kind(&self) -> HostActivationAdapterKind {
        HostActivationAdapterKind::Command
    }
}

fn read_env(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn bound(variable: &'static str, default: u64) -> Result<u64, HostActivationConfigError> {
    let Some(raw) = read_env(variable) else {
        return Ok(default);
    };
    raw.parse::<u64>().ok().filter(|value| *value > 0).ok_or(
        HostActivationConfigError::InvalidBound {
            variable,
            value: raw,
        },
    )
}

async fn capture<R>(source: Option<R>, limit: usize) -> Vec<u8>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(source) = source else {
        return Vec::new();
    };
    let mut buffer = Vec::new();
    let _ = source
        .take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_to_end(&mut buffer)
        .await;
    buffer
}

fn failed(reason: &str) -> Result<HostActivationOutcome, DomainError> {
    Ok(HostActivationOutcome::Failed(DeliveryFailureReason::new(
        reason,
    )?))
}

/// The head of what the command printed, made safe to store.
///
/// A transport reference is for correlating a wake-up with whatever the
/// host called it, so it is the command's word verbatim as far as it
/// fits, with control characters dropped rather than the whole
/// reference refused: a receipt is worth more than a tidy string.
fn transport_ref(stdout: &[u8]) -> Option<HostTransportRef> {
    summary(stdout).and_then(|value| HostTransportRef::new(value).ok())
}

fn summary(output: &[u8]) -> Option<String> {
    let head = &output[..TRANSPORT_REF_BYTES.min(output.len())];
    let readable = match std::str::from_utf8(head) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(&head[..error.valid_up_to()]).unwrap_or_default(),
    };
    let cleaned: String = readable
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests;
