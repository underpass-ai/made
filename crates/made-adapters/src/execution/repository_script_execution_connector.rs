use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorOutcome, CeremonyExecutionConnectorPort,
    CeremonyExecutionObservation, CeremonyExecutionRequest,
};
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
    ExternalOperationId,
};
use tokio::process::Command;
use uuid::Uuid;

use super::repository_script_execution_result::RepositoryScriptExecutionResult;

/// Runs an explicitly configured repository script with a durable operation protocol.
#[derive(Debug)]
pub struct RepositoryScriptExecutionConnector {
    id: ExecutionConnectorId,
    repository_root: PathBuf,
    executable: PathBuf,
    operation_root: PathBuf,
    timeout: Duration,
}

impl RepositoryScriptExecutionConnector {
    pub fn new(
        id: ExecutionConnectorId,
        repository_root: impl AsRef<Path>,
        executable: impl AsRef<Path>,
        operation_root: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Result<Self, DomainError> {
        if timeout.is_zero() {
            return Err(DomainError::MustBeNonZero {
                field: "repository_script_timeout",
            });
        }
        let repository_root = repository_root.as_ref().canonicalize().map_err(|_| {
            DomainError::InvariantViolated {
                reason: "repository script root is unavailable",
            }
        })?;
        let executable = repository_root
            .join(executable.as_ref())
            .canonicalize()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "repository script executable is unavailable",
            })?;
        if !executable.starts_with(&repository_root) || !executable.is_file() {
            return Err(DomainError::InvariantViolated {
                reason: "repository script executable must be a file inside its authorized root",
            });
        }
        let operation_root = operation_root.into();
        fs::create_dir_all(&operation_root).map_err(|_| DomainError::InvariantViolated {
            reason: "repository script connector cannot create its operation root",
        })?;
        let operation_root =
            operation_root
                .canonicalize()
                .map_err(|_| DomainError::InvariantViolated {
                    reason: "repository script operation root is unavailable",
                })?;
        Ok(Self {
            id,
            repository_root,
            executable,
            operation_root,
            timeout,
        })
    }

    fn request_path(&self, intent: &ExecutionIntent) -> PathBuf {
        self.operation_root.join(format!(
            "{}.request",
            intent.operation().operation_id().as_str()
        ))
    }

    fn result_path(&self, intent: &ExecutionIntent) -> PathBuf {
        self.operation_root.join(format!(
            "{}.result.json",
            intent.operation().operation_id().as_str()
        ))
    }

    fn admission_path(&self, intent: &ExecutionIntent) -> PathBuf {
        self.operation_root.join(format!(
            "{}.admitted",
            intent.operation().operation_id().as_str()
        ))
    }

    fn admit(path: &Path, operation_id: &str) -> Result<bool, DomainError> {
        let parent = path.parent().ok_or(DomainError::InvariantViolated {
            reason: "repository script admission has no operation root",
        })?;
        let mut marker = match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(marker) => marker,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(_) => {
                return Err(DomainError::InvariantViolated {
                    reason: "repository script connector cannot record admission",
                })
            }
        };
        marker
            .write_all(operation_id.as_bytes())
            .and_then(|()| marker.sync_all())
            .and_then(|()| File::open(parent)?.sync_all())
            .map_err(|_| DomainError::InvariantViolated {
                reason: "repository script connector cannot sync admission",
            })?;
        Ok(true)
    }

    fn persist_request(path: &Path, bytes: &[u8]) -> Result<(), DomainError> {
        if path.exists() {
            let stored = fs::read(path).map_err(|_| DomainError::InvariantViolated {
                reason: "repository script connector cannot read its sealed request",
            })?;
            return if stored == bytes {
                Ok(())
            } else {
                Err(DomainError::Conflict {
                    what: "repository_script_request",
                })
            };
        }
        let parent = path.parent().ok_or(DomainError::InvariantViolated {
            reason: "repository script request has no operation root",
        })?;
        let temporary_path = parent.join(format!(".request.{}.tmp", Uuid::new_v4()));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::hard_link(&temporary_path, path)?;
            File::open(parent)?.sync_all()
        })();
        let _ = fs::remove_file(&temporary_path);
        match write_result {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Self::persist_request(path, bytes)
            }
            Err(_) => Err(DomainError::InvariantViolated {
                reason: "repository script connector cannot seal its request",
            }),
        }
    }

    fn read_result(
        path: &Path,
        intent: &ExecutionIntent,
    ) -> Result<Option<CeremonyExecutionObservation>, DomainError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => {
                return Err(DomainError::InvariantViolated {
                    reason: "repository script connector cannot read its durable result",
                })
            }
        };
        let result: RepositoryScriptExecutionResult =
            serde_json::from_slice(&bytes).map_err(|_| DomainError::InvariantViolated {
                reason: "repository script durable result is unreadable",
            })?;
        result.validate(intent)?;
        Ok(Some(result.into_observation()))
    }

    async fn query(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<Option<CeremonyExecutionObservation>, DomainError> {
        let path = self.result_path(intent);
        let intent = intent.clone();
        tokio::task::spawn_blocking(move || Self::read_result(&path, &intent))
            .await
            .map_err(|_| DomainError::InvariantViolated {
                reason: "repository script result query task failed",
            })?
    }

    async fn execute_once(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        if let Some(observation) = self.query(intent).await? {
            return Self::observed_with_identity(intent, observation);
        }
        let request_path = self.request_path(intent);
        let request_bytes = intent.operation().request().as_bytes().to_vec();
        let sealed_path = request_path.clone();
        tokio::task::spawn_blocking(move || Self::persist_request(&sealed_path, &request_bytes))
            .await
            .map_err(|_| DomainError::InvariantViolated {
                reason: "repository script request task failed",
            })??;
        let admission_path = self.admission_path(intent);
        let operation_id = intent.operation().operation_id().as_str().to_owned();
        let admitted = tokio::task::spawn_blocking(move || {
            Self::admit(&admission_path, operation_id.as_str())
        })
        .await
        .map_err(|_| DomainError::InvariantViolated {
            reason: "repository script admission task failed",
        })??;
        if !admitted {
            return match self.query(intent).await? {
                Some(observation) => Self::observed_with_identity(intent, observation),
                None => Ok(Self::reconciliation_required(intent)),
            };
        }
        let result_path = self.result_path(intent);
        let mut command = Command::new(&self.executable);
        command
            .current_dir(&self.repository_root)
            .arg(intent.operation().operation_id().as_str())
            .arg(intent.operation().request_digest().as_str())
            .arg(intent.claim_fence().as_str())
            .arg(&request_path)
            .arg(&result_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let Ok(Ok(status)) = tokio::time::timeout(self.timeout, command.status()).await else {
            return Ok(Self::reconciliation_required(intent));
        };
        if !status.success() {
            return Ok(Self::reconciliation_required(intent));
        }
        match self.query(intent).await? {
            Some(observation) => Self::observed_with_identity(intent, observation),
            None => Ok(Self::reconciliation_required(intent)),
        }
    }

    fn external_operation_id(intent: &ExecutionIntent) -> Result<ExternalOperationId, DomainError> {
        ExternalOperationId::new(format!(
            "repository-script:{}",
            intent.operation().operation_id().as_str()
        ))
    }

    fn reconciliation_required(intent: &ExecutionIntent) -> CeremonyExecutionConnectorOutcome {
        CeremonyExecutionConnectorOutcome::ReconciliationRequired(
            intent.operation().operation_id().clone(),
        )
    }

    fn observed_with_identity(
        intent: &ExecutionIntent,
        observation: CeremonyExecutionObservation,
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        let (producer_claim_fence, _, result, artifacts, measured, observed_at) =
            observation.into_parts();
        Ok(CeremonyExecutionConnectorOutcome::Observed(Box::new(
            CeremonyExecutionObservation::new(
                producer_claim_fence,
                Some(Self::external_operation_id(intent)?),
                result,
                artifacts,
                observed_at,
            )
            .with_budget_measurement(measured),
        )))
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for RepositoryScriptExecutionConnector {
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
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        self.execute_once(request.intent()).await
    }

    async fn recover_intent(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        match self.query(intent).await? {
            Some(observation) => Self::observed_with_identity(intent, observation),
            None => Ok(Self::reconciliation_required(intent)),
        }
    }
}
