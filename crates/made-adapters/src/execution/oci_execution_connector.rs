use super::operation_record::{invalid, key, seal, OperationRecord};
use super::{OciExecutionConfig, OciExecutionRequest};
use async_trait::async_trait;
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
    StepClaimFence, StepErrorMessage, StepOutput, StepResult,
};
use made_core::{
    ports::{
        CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
        CeremonyExecutionRequest, ExecutionCancellation,
    },
    DomainError,
};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use time::OffsetDateTime;
use tokio::process::Command;

/// Docker-backed Linux execution. The daemon is trusted; the workload is not.
/// Containers have no host credentials, daemon socket or host PID/network namespace.
#[derive(Debug)]
pub struct OciExecutionConnector {
    id: ExecutionConnectorId,
    config: OciExecutionConfig,
}

impl OciExecutionConnector {
    pub fn new(id: ExecutionConnectorId, config: OciExecutionConfig) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            config: config.validate()?,
        })
    }
    fn name(intent: &ExecutionIntent) -> String {
        format!("made-oci-{}", key(intent))
    }
    fn path(&self, intent: &ExecutionIntent, suffix: &str) -> PathBuf {
        self.config
            .operation_root
            .join(format!("{}.{}", key(intent), suffix))
    }
    fn validate(&self, intent: &ExecutionIntent) -> Result<(), DomainError> {
        intent.validate()?;
        if intent.connector_id() != &self.id
            || intent.recovery_capability() != self.recovery_capability()
        {
            return Err(invalid("OCI connector identity mismatch"));
        }
        Ok(())
    }
    async fn docker(args: &[&str]) -> Result<std::process::Output, DomainError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Command::new("docker")
                .args(args)
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| invalid("Docker control timed out"))?
        .map_err(|_| invalid("Docker control unavailable"))
    }
    async fn terminate(&self, name: &str) -> Result<(), DomainError> {
        let output = Self::docker(&["kill", name]).await?;
        if !output.status.success() {
            let state = Self::docker(&["inspect", "--format", "{{.State.Running}}", name]).await?;
            if !state.status.success() || state.stdout != b"false\n" {
                return Err(invalid("OCI termination could not be confirmed"));
            }
        }
        Ok(())
    }
    async fn inspect(&self, name: &str) -> Result<Option<serde_json::Value>, DomainError> {
        let output = Self::docker(&["inspect", name]).await?;
        if !output.status.success() {
            return Ok(None);
        }
        let values: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|_| invalid("OCI state unreadable"))?;
        values
            .as_array()
            .and_then(|items| items.first())
            .cloned()
            .map(Some)
            .ok_or_else(|| invalid("OCI state unreadable"))
    }
    fn validate_state(
        intent: &ExecutionIntent,
        state: &serde_json::Value,
    ) -> Result<(), DomainError> {
        if state["Config"]["Labels"]["made.digest"].as_str()
            != Some(intent.operation().request_digest().as_str())
        {
            return Err(DomainError::Conflict {
                what: "OCI operation digest",
            });
        }
        if state["Config"]["Labels"]["made.fence"].as_str() != Some(intent.claim_fence().as_str()) {
            return Err(DomainError::Conflict {
                what: "OCI producer claim fence",
            });
        }
        Ok(())
    }
    fn persist_failure(
        &self,
        intent: &ExecutionIntent,
        name: String,
        reason: &'static str,
    ) -> Result<Outcome, DomainError> {
        let record = OperationRecord {
            digest: intent.operation().request_digest().as_str().to_owned(),
            fence: intent.claim_fence().clone(),
            external_id: name,
            result: StepResult::failed(StepErrorMessage::new(reason)?)?,
            observed_at: OffsetDateTime::now_utc(),
        };
        record.persist(&self.path(intent, "result"))?;
        Ok(Outcome::Observed(Box::new(record.observe(intent)?)))
    }
    async fn query(&self, intent: &ExecutionIntent) -> Result<Outcome, DomainError> {
        self.validate(intent)?;
        if let Some(record) = OperationRecord::load(&self.path(intent, "result"))? {
            return Ok(Outcome::Observed(Box::new(record.observe(intent)?)));
        }
        let name = Self::name(intent);
        let Some(state) = self.inspect(&name).await? else {
            return Ok(Outcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        };
        Self::validate_state(intent, &state)?;
        if state["State"]["Running"].as_bool() == Some(true) {
            self.terminate(&name).await?;
            return self.persist_failure(intent, name, "OCI orphan was terminated during recovery");
        }
        if state["State"]["Status"].as_str() == Some("created") {
            return Ok(Outcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        }
        let fence = StepClaimFence::new(
            state["Config"]["Labels"]["made.fence"]
                .as_str()
                .ok_or_else(|| invalid("OCI fence missing"))?,
        )?;
        let success = state["State"]["ExitCode"].as_i64() == Some(0);
        let result = if success {
            StepResult::completed(StepOutput::empty())?
        } else {
            StepResult::failed(StepErrorMessage::new("OCI process failed")?)?
        };
        let record = OperationRecord {
            digest: intent.operation().request_digest().as_str().to_owned(),
            fence,
            external_id: name,
            result,
            observed_at: OffsetDateTime::now_utc(),
        };
        record.persist(&self.path(intent, "result"))?;
        Ok(Outcome::Observed(Box::new(record.observe(intent)?)))
    }
    fn request(intent: &ExecutionIntent) -> Result<OciExecutionRequest, DomainError> {
        let value: serde_json::Value =
            serde_json::from_slice(intent.operation().request().as_bytes())
                .map_err(|_| invalid("OCI semantic request invalid"))?;
        let request: OciExecutionRequest =
            serde_json::from_value(value["handler_config"]["oci"].clone())
                .map_err(|_| invalid("OCI request missing handler_config.oci"))?;
        if request.argv.is_empty() || request.argv[0].is_empty() {
            return Err(invalid("OCI argv is empty"));
        }
        Ok(request)
    }

    fn create_arguments(
        &self,
        intent: &ExecutionIntent,
        name: &str,
        request: OciExecutionRequest,
    ) -> Vec<String> {
        let mount = format!(
            "type=bind,src={},dst=/workspace",
            self.config.workspace.display()
        );
        let mut args = vec![
            "create".to_owned(),
            "--name".into(),
            name.to_owned(),
            "--pull=never".into(),
            "--user".into(),
            format!("{}:{}", self.config.uid, self.config.uid),
            "--cap-drop=ALL".into(),
            "--security-opt=no-new-privileges".into(),
            "--read-only".into(),
            "--network".into(),
            self.config.network.clone(),
            "--cpus".into(),
            self.config.cpus.to_string(),
            "--memory".into(),
            self.config.memory_bytes.to_string(),
            "--memory-swap".into(),
            self.config.memory_bytes.to_string(),
            "--pids-limit".into(),
            self.config.pids.to_string(),
            "--ulimit".into(),
            "core=0".into(),
            "--log-driver=none".into(),
            "--mount".into(),
            mount,
            "--workdir=/workspace".into(),
            "--label".into(),
            format!(
                "made.digest={}",
                intent.operation().request_digest().as_str()
            ),
            "--label".into(),
            format!("made.fence={}", intent.claim_fence().as_str()),
            "--label".into(),
            format!(
                "made.timeout-seconds={}",
                watchdog_seconds(self.config.timeout)
            ),
            self.config.image.clone(),
            "/usr/bin/timeout".into(),
            "--signal=KILL".into(),
            "--kill-after=1s".into(),
            format!("{}s", watchdog_seconds(self.config.timeout)),
        ];
        args.extend(request.argv);
        args
    }

    fn reconciliation(intent: &ExecutionIntent) -> Outcome {
        Outcome::ReconciliationRequired(intent.operation().operation_id().clone())
    }

    async fn prepare_container(
        &self,
        intent: &ExecutionIntent,
        name: &str,
        arguments: &[String],
        admitted: bool,
    ) -> Result<Option<Outcome>, DomainError> {
        if admitted {
            let Ok(create) =
                Self::docker(&arguments.iter().map(String::as_str).collect::<Vec<_>>()).await
            else {
                return self.query(intent).await.map(Some);
            };
            if !create.status.success() {
                let Some(state) = self.inspect(name).await? else {
                    return Ok(Some(Self::reconciliation(intent)));
                };
                Self::validate_state(intent, &state)?;
                if state["State"]["Status"].as_str() != Some("created") {
                    return self.query(intent).await.map(Some);
                }
            }
        } else {
            if OperationRecord::load(&self.path(intent, "result"))?.is_some() {
                return self.query(intent).await.map(Some);
            }
            let Some(state) = self.inspect(name).await? else {
                return Ok(Some(Self::reconciliation(intent)));
            };
            Self::validate_state(intent, &state)?;
            if state["State"]["Status"].as_str() != Some("created") {
                return self.query(intent).await.map(Some);
            }
        }
        Ok(None)
    }

    async fn execute_created(
        &self,
        intent: &ExecutionIntent,
        name: String,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        if cancellation.is_cancelled() {
            return self.persist_failure(
                intent,
                name,
                "execution authority was cancelled before OCI start",
            );
        }
        let mut child = Command::new("docker")
            .args(["start", "--attach", &name])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| invalid("OCI attach failed"))?;
        let (sender, mut overflow) = tokio::sync::mpsc::unbounded_channel();
        let total = Arc::new(AtomicUsize::new(0));
        let stdout = tokio::spawn(super::oci_output::drain(
            child
                .stdout
                .take()
                .ok_or_else(|| invalid("OCI stdout missing"))?,
            Arc::clone(&total),
            self.config.max_output_bytes,
            sender.clone(),
        ));
        let stderr = tokio::spawn(super::oci_output::drain(
            child
                .stderr
                .take()
                .ok_or_else(|| invalid("OCI stderr missing"))?,
            Arc::clone(&total),
            self.config.max_output_bytes,
            sender,
        ));
        let failure = tokio::select! {
            () = cancellation.cancelled() => Some("execution authority was cancelled"),
            () = tokio::time::sleep(self.config.timeout) => Some("OCI execution timed out"),
            Some(()) = overflow.recv() => Some("OCI output limit exceeded"),
            status = child.wait() => { status.map_err(|_| invalid("OCI attach wait failed"))?; None }
        };
        if let Some(reason) = failure {
            self.terminate(&name).await?;
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout.abort();
            stderr.abort();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let _ = stdout.await;
                let _ = stderr.await;
            })
            .await;
            return self.persist_failure(intent, name, reason);
        }
        let stdout = stdout
            .await
            .map_err(|_| invalid("OCI stdout drain task failed"))?;
        let stderr = stderr
            .await
            .map_err(|_| invalid("OCI stderr drain task failed"))?;
        if stdout.is_err() || stderr.is_err() {
            return self.persist_failure(intent, name, "OCI output drain failed");
        }
        if total.load(Ordering::Relaxed) > self.config.max_output_bytes {
            return self.persist_failure(intent, name, "OCI output limit exceeded");
        }
        self.query(intent).await
    }

    async fn run(
        &self,
        intent: &ExecutionIntent,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        self.validate(intent)?;
        if cancellation.is_cancelled() {
            return Err(invalid("execution authority was cancelled"));
        }
        let request = Self::request(intent)?;
        // The sealed admission survives response/receipt loss. Never restart an admitted effect.
        let admitted = seal(
            &self.path(intent, "admitted"),
            intent.operation().request_digest().as_str().as_bytes(),
        )?;
        let name = Self::name(intent);
        let arguments = self.create_arguments(intent, &name, request);
        if let Some(outcome) = self
            .prepare_container(intent, &name, &arguments, admitted)
            .await?
        {
            return Ok(outcome);
        }
        self.execute_created(intent, name, cancellation).await
    }
}

fn watchdog_seconds(timeout: std::time::Duration) -> u64 {
    timeout
        .as_secs()
        .saturating_add(u64::from(timeout.subsec_nanos() != 0))
}

#[async_trait]
impl CeremonyExecutionConnectorPort for OciExecutionConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }
    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        ExecutionRecoveryCapability::QueryableByOperationId
    }
    fn source_kind(&self) -> ArtifactSourceKind {
        ArtifactSourceKind::ExternalExecution
    }
    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<Outcome, DomainError> {
        self.run(request.intent(), ExecutionCancellation::new())
            .await
    }
    async fn execute_cancellable(
        &self,
        request: CeremonyExecutionRequest,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        self.run(request.intent(), cancellation).await
    }
    async fn recover_intent(&self, intent: &ExecutionIntent) -> Result<Outcome, DomainError> {
        self.query(intent).await
    }
}
