#![cfg(feature = "sqlite")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::execution::{
    DurableFixtureExecutionConnector, RepositoryScriptExecutionConnector,
};
use made_adapters::memory::{
    ForgetfulMemory, InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    MountCeremonyDefinitionsUseCase, ResolveCeremonyDefinitionUseCase, StartCeremonyInput,
    StartCeremonyStepInput, StartCeremonyStepUseCase, StartCeremonyUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, ExecuteCeremonyOperationInput,
    ExecuteCeremonyOperationUseCase, InspectExecutionRecoveryUseCase,
    RecoverExecutionIntentOutcome, RecoverExecutionIntentUseCase, RecoverableCeremonyWorker,
    RecoverableCeremonyWorkerOutcome,
};
use made_core::ports::{
    CeremonyExecutionConnectorOutcome, CeremonyExecutionConnectorPort, CeremonyExecutionRequest,
    ExecutionReceiptStorePort, NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId, ExecutionIntent,
    ExecutionOperation, ExecutionRecoveryCapability, ExecutionRecoveryPageLimit,
    ExecutionRequestBytes, IdempotencyKey, LeaseOwnerId, RoleId, StateIteration, StateVisit,
    StepClaimFence, StepHandlerConfig, StepHandlerKind, StepId, StepIteration, StepOutput,
    StepResult, StepStatus,
};
use time::OffsetDateTime;

const DEFINITION: &str = r#"
version: "1.0"
name: "durable_worker"
description: "Recoverable worker fixture"
inputs:
  required: []
  optional: []
outputs: {}
states:
  - id: OPEN
    initial: true
    terminal: false
steps:
  - id: work
    state: OPEN
    handler: fixture
    config: {}
roles:
  - id: WORKER
    allowed_actions:
      - work
retry_policies:
  default:
    max_attempts: 2
    backoff_seconds: 1
"#;

fn scratch() -> tempfile::TempDir {
    std::fs::create_dir_all("tmp").unwrap();
    tempfile::tempdir_in("tmp").unwrap()
}

struct FailAfterEffectConnector {
    inner: Arc<dyn CeremonyExecutionConnectorPort>,
    fail_once: AtomicBool,
}

impl std::fmt::Debug for FailAfterEffectConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailAfterEffectConnector")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for FailAfterEffectConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        self.inner.connector_id()
    }

    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        self.inner.recovery_capability()
    }

    fn source_kind(&self) -> ArtifactSourceKind {
        self.inner.source_kind()
    }

    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<CeremonyExecutionConnectorOutcome, made_core::DomainError> {
        let outcome = self.inner.execute_or_recover(request).await?;
        if matches!(outcome, CeremonyExecutionConnectorOutcome::Observed(_))
            && self.fail_once.swap(false, Ordering::SeqCst)
        {
            return Err(made_core::DomainError::InvariantViolated {
                reason: "injected process exit after durable effect",
            });
        }
        Ok(outcome)
    }

    async fn recover_intent(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionConnectorOutcome, made_core::DomainError> {
        self.inner.recover_intent(intent).await
    }
}

fn intent() -> ExecutionIntent {
    ExecutionIntent::new(
        ExecutionOperation::new(
            CeremonyId::new("restart-ceremony").unwrap(),
            StepId::new("work").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            ExecutionRequestBytes::new(b"sealed durable fixture request".to_vec()).unwrap(),
        ),
        StepClaimFence::new("1".repeat(64)).unwrap(),
        ExecutionConnectorId::new("fixture.filesystem.v1").unwrap(),
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::Fixture,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

#[tokio::test]
async fn a_restarted_worker_recovers_an_effect_written_before_its_receipt() {
    let directory = scratch();
    let database = directory.path().join("ceremonies.sqlite3");
    let effects = directory.path().join("effects");
    let intent = intent();
    let result = StepResult::completed(StepOutput::empty()).unwrap();
    {
        let store = SqliteCeremonyStore::open(&database).unwrap();
        store.record_intent(intent.clone()).await.unwrap();
        let connector = DurableFixtureExecutionConnector::new(
            &effects,
            result.clone(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        assert!(matches!(
            connector.recover_intent(&intent).await.unwrap(),
            CeremonyExecutionConnectorOutcome::Observed(_)
        ));
        assert_eq!(
            store
                .receipt(intent.operation().operation_id())
                .await
                .unwrap(),
            None
        );
    }

    let reopened: Arc<dyn ExecutionReceiptStorePort> =
        Arc::new(SqliteCeremonyStore::open(&database).unwrap());
    let connector: Arc<dyn CeremonyExecutionConnectorPort> = Arc::new(
        DurableFixtureExecutionConnector::new(&effects, result, OffsetDateTime::UNIX_EPOCH)
            .unwrap(),
    );
    let recover = RecoverExecutionIntentUseCase::new(reopened.clone(), connector);
    let outcome = recover.execute(&intent).await.unwrap();
    let RecoverExecutionIntentOutcome::Receipt(receipt) = outcome else {
        panic!("a durable fixture effect is automatically recoverable");
    };

    assert_eq!(receipt.producer_claim_fence(), intent.claim_fence());
    assert_eq!(receipt.source_kind(), ArtifactSourceKind::Fixture);
    assert_eq!(
        reopened
            .receipt(intent.operation().operation_id())
            .await
            .unwrap(),
        Some(*receipt)
    );
    assert_eq!(
        std::fs::read_dir(&effects)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .count(),
        1
    );
}

#[cfg(unix)]
fn write_repository_worker(repository: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let script_path = repository.join("worker.py");
    std::fs::write(
        &script_path,
        r#"#!/usr/bin/env python3
import json
import os
import pathlib
import sys

operation_id, request_digest, claim_fence, request_path, result_path = sys.argv[1:]
repository = pathlib.Path(__file__).parent
with (repository / "invocations.log").open("a", encoding="utf-8") as log:
    log.write(operation_id + "\n")
    log.flush()
    os.fsync(log.fileno())
temporary_effect = repository / ("." + operation_id + ".effect.tmp")
temporary_effect.write_bytes(pathlib.Path(request_path).read_bytes())
os.replace(temporary_effect, repository / "materialized-request.json")
result = {
    "operation_id": operation_id,
    "request_digest": request_digest,
    "producer_claim_fence": claim_fence,
    "result": {"status": "COMPLETED", "output": {}},
    "observed_at": "1970-01-01T00:00:00Z",
}
result_path = pathlib.Path(result_path)
temporary_result = result_path.parent / ("." + operation_id + ".result.tmp")
with temporary_result.open("w", encoding="utf-8") as output:
    json.dump(result, output, separators=(",", ":"))
    output.flush()
    os.fsync(output.fileno())
os.replace(temporary_result, result_path)
directory = os.open(result_path.parent, os.O_RDONLY)
try:
    os.fsync(directory)
finally:
    os.close(directory)
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(script_path, permissions).unwrap();
}

#[cfg(unix)]
async fn leave_real_effect_without_receipt(
    database: &std::path::Path,
    repository: &std::path::Path,
    operation_root: &std::path::Path,
    definitions: Arc<InMemoryCeremonyDefinitionRepository>,
    clock: Arc<SystemClock>,
    ceremony_id: &CeremonyId,
    step_id: &StepId,
) {
    let role_id = RoleId::new("WORKER").unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    StartCeremonyUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartCeremonyInput::new(
        ceremony_id.clone(),
        made_core::value_objects::CeremonyName::new("durable_worker").unwrap(),
        made_core::value_objects::CeremonyVersion::v1(),
        made_core::value_objects::CeremonyContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryCeremonyDefinitionPublications::new()),
    ));
    let claim = StartCeremonyStepUseCase::new(resolver, stream, clock.clone())
        .execute(StartCeremonyStepInput::new(
            ceremony_id.clone(),
            role_id.clone(),
            AuditActorKind::Agent,
            step_id.clone(),
            LeaseOwnerId::new("worker-1").unwrap(),
            IdempotencyKey::new("driver-restart-work-1").unwrap(),
            made_core::value_objects::DurationMs::from_millis(60_000),
        ))
        .await
        .unwrap();
    let record = claim.instance().step_record(step_id).unwrap();
    let handler_request = made_core::ports::CeremonyStepHandlerRequest::new(
        ceremony_id.clone(),
        made_core::value_objects::CeremonyName::new("durable_worker").unwrap(),
        made_core::value_objects::CeremonyVersion::v1(),
        claim.instance().current_state().clone(),
        step_id.clone(),
        StepHandlerKind::new("fixture").unwrap(),
        StepHandlerConfig::empty(),
        claim.instance().context().clone(),
        claim.attempt(),
    )
    .with_role(role_id);
    let durable: Arc<dyn CeremonyExecutionConnectorPort> = Arc::new(
        RepositoryScriptExecutionConnector::new(
            ExecutionConnectorId::new("local.repository-script.driver").unwrap(),
            repository,
            "worker.py",
            operation_root,
            Duration::from_secs(5),
        )
        .unwrap(),
    );
    let connector: Arc<dyn CeremonyExecutionConnectorPort> = Arc::new(FailAfterEffectConnector {
        inner: durable,
        fail_once: AtomicBool::new(true),
    });
    let receipts: Arc<dyn ExecutionReceiptStorePort> = store;
    let execute = ExecuteCeremonyOperationUseCase::new(receipts, connector, clock);
    assert!(execute
        .execute(ExecuteCeremonyOperationInput {
            handler_request,
            state_visit: record.state_visit(),
            state_iteration: record.state_iteration(),
            step_iteration: record.iteration(),
            claim_fence: claim.claim_fence().clone(),
            actor_kind: AuditActorKind::Agent,
        })
        .await
        .is_err());
}

#[cfg(unix)]
async fn recover_real_effect(
    database: &std::path::Path,
    repository: &std::path::Path,
    operation_root: &std::path::Path,
    definitions: Arc<InMemoryCeremonyDefinitionRepository>,
    clock: Arc<SystemClock>,
) -> RecoverableCeremonyWorkerOutcome {
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryCeremonyDefinitionPublications::new()),
    ));
    let connector: Arc<dyn CeremonyExecutionConnectorPort> = Arc::new(
        RepositoryScriptExecutionConnector::new(
            ExecutionConnectorId::new("local.repository-script.driver").unwrap(),
            repository,
            "worker.py",
            operation_root,
            Duration::from_secs(5),
        )
        .unwrap(),
    );
    let receipts: Arc<dyn ExecutionReceiptStorePort> = store.clone();
    let inspect = InspectExecutionRecoveryUseCase::new(stream.clone(), receipts.clone());
    let page = inspect
        .execute(None, ExecutionRecoveryPageLimit::new(10).unwrap())
        .await
        .unwrap();
    assert_eq!(page.items().len(), 1);
    let item = page.into_parts().0.into_iter().next().unwrap();
    assert!(item.receipt().is_none());
    let worker = RecoverableCeremonyWorker::new(
        Arc::new(ExecuteCeremonyOperationUseCase::new(
            receipts.clone(),
            connector.clone(),
            clock.clone(),
        )),
        Arc::new(RecoverExecutionIntentUseCase::new(
            receipts.clone(),
            connector,
        )),
        Arc::new(CompleteExecutionReceiptUseCase::new(
            resolver, stream, receipts, clock,
        )),
    );
    worker.recover(item).await.unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn the_reopened_driver_queries_real_work_then_links_and_completes() {
    let directory = scratch();
    let database = directory.path().join("driver.sqlite3");
    let operation_root = directory.path().join("driver-operations");
    let repository = directory.path().join("authorized-repository");
    std::fs::create_dir_all(&repository).unwrap();
    write_repository_worker(&repository);
    let definitions_dir = directory.path().join("definitions");
    std::fs::create_dir_all(&definitions_dir).unwrap();
    std::fs::write(definitions_dir.join("durable_worker.yaml"), DEFINITION).unwrap();
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(&definitions_dir).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let clock = Arc::new(SystemClock::new());
    let ceremony_id = CeremonyId::new("driver-restart").unwrap();
    let step_id = StepId::new("work").unwrap();
    leave_real_effect_without_receipt(
        &database,
        &repository,
        &operation_root,
        definitions.clone(),
        clock.clone(),
        &ceremony_id,
        &step_id,
    )
    .await;
    let outcome =
        recover_real_effect(&database, &repository, &operation_root, definitions, clock).await;
    let RecoverableCeremonyWorkerOutcome::Completed { instance, receipt } = outcome else {
        panic!("the repository script connector has authoritative recovery");
    };

    assert_eq!(receipt.source_kind(), ArtifactSourceKind::ExternalExecution);
    assert_eq!(
        instance.step_record(&step_id).unwrap().status(),
        StepStatus::Completed
    );
    assert!(instance
        .execution_receipt_link(receipt.operation_id())
        .is_some());
    assert_eq!(
        std::fs::read_to_string(repository.join("invocations.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(repository.join("materialized-request.json").is_file());
}
