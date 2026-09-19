use super::GitExecutionRequest;
use crate::execution::operation_record::{invalid, key, OperationRecord};
use async_trait::async_trait;
use made_core::value_objects::{
    ArtifactSourceKind, Attributes, ExecutionConnectorId, ExecutionIntent,
    ExecutionRecoveryCapability, StepOutput, StepResult,
};
use made_core::{
    ports::{
        CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
        CeremonyExecutionRequest, ExecutionCancellation,
    },
    DomainError,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::{io::AsyncWriteExt, process::Command};

/// Atomic local Git publication into an explicitly configured bare repository.
/// The target ref CAS and the immutable logical-operation ref commit together.
#[derive(Debug)]
pub struct GitExecutionConnector {
    id: ExecutionConnectorId,
    repository: PathBuf,
    scratch: PathBuf,
}

impl GitExecutionConnector {
    pub fn new(
        id: ExecutionConnectorId,
        repository: impl AsRef<Path>,
        scratch: impl AsRef<Path>,
    ) -> Result<Self, DomainError> {
        let repository = repository
            .as_ref()
            .canonicalize()
            .map_err(|_| invalid("Git repository unavailable"))?;
        std::fs::create_dir_all(scratch.as_ref())
            .map_err(|_| invalid("Git scratch unavailable"))?;
        let scratch = scratch
            .as_ref()
            .canonicalize()
            .map_err(|_| invalid("Git scratch unavailable"))?;
        Ok(Self {
            id,
            repository,
            scratch,
        })
    }
    async fn git(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
        index: Option<&Path>,
    ) -> Result<std::process::Output, DomainError> {
        let mut command = Command::new("git");
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "MADE")
            .env("GIT_AUTHOR_EMAIL", "made@localhost")
            .env("GIT_COMMITTER_NAME", "MADE")
            .env("GIT_COMMITTER_EMAIL", "made@localhost")
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
            .arg("--git-dir")
            .arg(&self.repository)
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        let mut child = command.spawn().map_err(|_| invalid("Git unavailable"))?;
        if let Some(bytes) = input {
            child
                .stdin
                .take()
                .ok_or_else(|| invalid("Git stdin unavailable"))?
                .write_all(bytes)
                .await
                .map_err(|_| invalid("Git input failed"))?;
        }
        tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output())
            .await
            .map_err(|_| invalid("Git timed out"))?
            .map_err(|_| invalid("Git failed"))
    }
    async fn require_git(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
        index: Option<&Path>,
    ) -> Result<String, DomainError> {
        let result = self.git(args, input, index).await?;
        if !result.status.success() {
            return Err(DomainError::Conflict {
                what: "git_expected_change",
            });
        }
        String::from_utf8(result.stdout)
            .map(|s| s.trim().to_owned())
            .map_err(|_| invalid("Git output invalid"))
    }
    fn request(&self, intent: &ExecutionIntent) -> Result<GitExecutionRequest, DomainError> {
        intent.validate()?;
        if intent.connector_id() != &self.id
            || intent.recovery_capability() != self.recovery_capability()
        {
            return Err(invalid("Git connector identity mismatch"));
        }
        let json: serde_json::Value =
            serde_json::from_slice(intent.operation().request().as_bytes())
                .map_err(|_| invalid("Git semantic request invalid"))?;
        let request: GitExecutionRequest =
            serde_json::from_value(json["handler_config"]["git"].clone())
                .map_err(|_| invalid("Git request missing handler_config.git"))?;
        let authorized_repository = Path::new(&request.repository)
            .canonicalize()
            .map_err(|_| invalid("Git requested repository unavailable"))?;
        if authorized_repository != self.repository {
            return Err(DomainError::Conflict {
                what: "git_authorized_repository",
            });
        }
        if !request.branch.starts_with("refs/heads/") {
            return Err(DomainError::Conflict {
                what: "git_authorized_branch",
            });
        }
        if request.expected_tip.len() != 40
            || !request.expected_tip.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(DomainError::Conflict {
                what: "git_expected_tip",
            });
        }
        if format!("{:x}", Sha256::digest(request.diff.as_bytes())) != request.diff_sha256
            || request.diff.len() > 4 * 1024 * 1024
        {
            return Err(DomainError::Conflict {
                what: "git_change_digest",
            });
        }
        Ok(request)
    }
    async fn query(&self, intent: &ExecutionIntent) -> Result<Outcome, DomainError> {
        let request = self.request(intent)?;
        let reference = format!("refs/made/operations/{}", key(intent));
        let output = self
            .git(&["show", "-s", "--format=%B", &reference], None, None)
            .await?;
        if !output.status.success() {
            return Ok(Outcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        }
        let mut record: OperationRecord = serde_json::from_slice(&output.stdout)
            .map_err(|_| invalid("Git operation metadata corrupt"))?;
        let parent = self
            .require_git(&["rev-parse", &format!("{reference}^")], None, None)
            .await?;
        if parent != request.expected_tip {
            return Err(DomainError::Conflict {
                what: "git_operation_parent",
            });
        }
        let commit = self
            .require_git(&["rev-parse", &reference], None, None)
            .await?;
        record.external_id = format!("git:{commit}");
        record.result = StepResult::completed(StepOutput::new(Attributes::new(
            std::collections::BTreeMap::from([("commit".to_owned(), serde_json::json!(commit))]),
        )?))?;
        Ok(Outcome::Observed(Box::new(record.observe(intent)?)))
    }
    async fn run(
        &self,
        intent: &ExecutionIntent,
        cancellation: ExecutionCancellation,
    ) -> Result<Outcome, DomainError> {
        let request = self.request(intent)?;
        if let observed @ Outcome::Observed(_) = self.query(intent).await? {
            return Ok(observed);
        }
        if cancellation.is_cancelled() {
            return Err(invalid("execution authority was cancelled"));
        }
        if self
            .require_git(&["rev-parse", "--is-bare-repository"], None, None)
            .await?
            != "true"
        {
            return Err(invalid("Git connector requires isolated bare repository"));
        }
        self.require_git(&["check-ref-format", &request.branch], None, None)
            .await?;
        let index = self.scratch.join(format!("index-{}", uuid::Uuid::new_v4()));
        let build = async {
            self.require_git(&["read-tree", &request.expected_tip], None, Some(&index))
                .await?;
            self.require_git(
                &["apply", "--cached", "--check", "--whitespace=error"],
                Some(request.diff.as_bytes()),
                Some(&index),
            )
            .await?;
            self.require_git(
                &["apply", "--cached", "--whitespace=error"],
                Some(request.diff.as_bytes()),
                Some(&index),
            )
            .await?;
            let tree = self
                .require_git(&["write-tree"], None, Some(&index))
                .await?;
            let record = OperationRecord {
                digest: intent.operation().request_digest().as_str().to_owned(),
                fence: intent.claim_fence().clone(),
                external_id: "git:pending".into(),
                result: StepResult::completed(StepOutput::empty())?,
                observed_at: time::OffsetDateTime::UNIX_EPOCH,
            };
            let message =
                serde_json::to_vec(&record).map_err(|_| invalid("Git metadata encoding failed"))?;
            self.require_git(
                &["commit-tree", &tree, "-p", &request.expected_tip],
                Some(&message),
                Some(&index),
            )
            .await
        }
        .await;
        let _ = std::fs::remove_file(&index);
        let commit = build?;
        if cancellation.is_cancelled() {
            return Err(invalid("execution authority was cancelled before Git CAS"));
        }
        let transaction = format!(
            "start\nupdate {} {} {}\ncreate refs/made/operations/{} {}\nprepare\ncommit\n",
            request.branch,
            commit,
            request.expected_tip,
            key(intent),
            commit
        );
        // No external hooks or network. One ref transaction is the linearization point.
        let output = self
            .git(
                &["update-ref", "--stdin"],
                Some(transaction.as_bytes()),
                None,
            )
            .await?;
        let observed = self.query(intent).await?;
        if !output.status.success() && !matches!(observed, Outcome::Observed(_)) {
            return Err(DomainError::Conflict {
                what: "git_expected_tip",
            });
        }
        Ok(observed)
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for GitExecutionConnector {
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
