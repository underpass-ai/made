use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorPort, CeremonyExecutionObservation, CeremonyExecutionRequest,
};
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
    ExternalOperationId, StepResult,
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::durable_fixture_effect::DurableFixtureEffect;

/// Filesystem reference connector with operation-id idempotency across processes.
#[derive(Debug)]
pub struct DurableFixtureExecutionConnector {
    id: ExecutionConnectorId,
    root: PathBuf,
    result: StepResult,
    observed_at: OffsetDateTime,
}

impl DurableFixtureExecutionConnector {
    pub fn new(
        root: impl Into<PathBuf>,
        result: StepResult,
        observed_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|_| DomainError::InvariantViolated {
            reason: "durable fixture connector cannot create its effect root",
        })?;
        Ok(Self {
            id: ExecutionConnectorId::new("fixture.filesystem.v1")?,
            root,
            result,
            observed_at,
        })
    }

    fn effect_path(root: &Path, intent: &ExecutionIntent) -> PathBuf {
        root.join(format!(
            "{}.json",
            intent.operation().operation_id().as_str()
        ))
    }

    fn settle(
        root: &Path,
        intent: &ExecutionIntent,
        result: StepResult,
        observed_at: OffsetDateTime,
    ) -> Result<DurableFixtureEffect, DomainError> {
        let effect_path = Self::effect_path(root, intent);
        if effect_path.exists() {
            return Self::read_effect(&effect_path, intent);
        }
        let effect = DurableFixtureEffect::new(intent, result, observed_at);
        let bytes = serde_json::to_vec(&effect).map_err(|_| DomainError::InvariantViolated {
            reason: "durable fixture connector cannot encode its effect",
        })?;
        let temporary_path = root.join(format!(
            ".{}.{}.tmp",
            intent.operation().operation_id().as_str(),
            Uuid::new_v4()
        ));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::hard_link(&temporary_path, &effect_path)?;
            File::open(root)?.sync_all()
        })();
        let _ = fs::remove_file(&temporary_path);
        match write_result {
            Ok(()) => Ok(effect),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Self::read_effect(&effect_path, intent)
            }
            Err(_) => Err(DomainError::InvariantViolated {
                reason: "durable fixture connector cannot persist its effect",
            }),
        }
    }

    fn read_effect(
        path: &Path,
        intent: &ExecutionIntent,
    ) -> Result<DurableFixtureEffect, DomainError> {
        let bytes = fs::read(path).map_err(|_| DomainError::InvariantViolated {
            reason: "durable fixture connector cannot read its effect",
        })?;
        let effect: DurableFixtureEffect =
            serde_json::from_slice(&bytes).map_err(|_| DomainError::InvariantViolated {
                reason: "durable fixture connector effect is unreadable",
            })?;
        effect.validate(intent)?;
        Ok(effect)
    }

    async fn settle_intent(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        let root = self.root.clone();
        let intent = intent.clone();
        let external_operation_id = ExternalOperationId::new(format!(
            "fixture:{}",
            intent.operation().operation_id().as_str()
        ))?;
        let result = self.result.clone();
        let observed_at = self.observed_at;
        let effect =
            tokio::task::spawn_blocking(move || Self::settle(&root, &intent, result, observed_at))
                .await
                .map_err(|_| DomainError::InvariantViolated {
                    reason: "durable fixture connector task failed",
                })??;
        let (producer_claim_fence, result, observed_at) = effect.into_parts();
        Ok(CeremonyExecutionObservation::new(
            producer_claim_fence,
            Some(external_operation_id),
            result,
            Vec::new(),
            observed_at,
        ))
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for DurableFixtureExecutionConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }

    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        ExecutionRecoveryCapability::IdempotentByOperationId
    }

    fn source_kind(&self) -> ArtifactSourceKind {
        ArtifactSourceKind::Fixture
    }

    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        self.settle_intent(request.intent()).await
    }

    async fn recover_intent(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        self.settle_intent(intent).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use made_core::ports::CeremonyExecutionConnectorPort;
    use made_core::value_objects::{
        AuditActorKind, CeremonyId, ExecutionIntent, ExecutionOperation, ExecutionRequestBytes,
        StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
    };

    use super::*;

    fn intent(fence: char) -> ExecutionIntent {
        ExecutionIntent::new(
            ExecutionOperation::new(
                CeremonyId::new("ceremony").unwrap(),
                StepId::new("work").unwrap(),
                StateVisit::FIRST,
                StateIteration::FIRST,
                StepIteration::FIRST,
                ExecutionRequestBytes::new(b"sealed request".to_vec()).unwrap(),
            ),
            StepClaimFence::new(fence.to_string().repeat(64)).unwrap(),
            ExecutionConnectorId::new("fixture.filesystem.v1").unwrap(),
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::Fixture,
            AuditActorKind::Engine,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn two_connector_instances_publish_one_atomic_effect() {
        let directory = tempfile::tempdir().unwrap();
        let result = StepResult::completed(made_core::value_objects::StepOutput::empty()).unwrap();
        let left = Arc::new(
            DurableFixtureExecutionConnector::new(
                directory.path(),
                result.clone(),
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
        );
        let right = Arc::new(
            DurableFixtureExecutionConnector::new(
                directory.path(),
                result,
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
        );
        let first = intent('1');
        let reclaimed = intent('2');

        let (left_observation, right_observation) = tokio::join!(
            left.recover_intent(&first),
            right.recover_intent(&reclaimed)
        );
        let left_observation = left_observation.unwrap();
        let right_observation = right_observation.unwrap();

        assert_eq!(
            left_observation.producer_claim_fence(),
            right_observation.producer_claim_fence()
        );
        assert_eq!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .count(),
            1
        );
    }
}
