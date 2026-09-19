use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use made::workers::CeremonyWorkerDaemon;
use made_adapters::clock::SystemClock;
use made_adapters::memory::{ForgetfulMemory, InMemoryCeremonyDefinitionRepository};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::workers::FileWorkerCapacityStore;
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceCeremonyDeadlinesUseCase, MountCeremonyDefinitionsUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepUseCase,
    StartCeremonyUseCase,
};
use made_app::workers::{
    CeremonyWorkerCapacity, CeremonyWorkerDriver, CeremonyWorkerHost, CeremonyWorkerHostPolicy,
    CeremonyWorkerPolicy, CeremonyWorkerRenewal, CeremonyWorkerStopToken, ClaimCeremonyWorkInput,
    ClaimCeremonyWorkUseCase, ExecuteCeremonyOperationInput, ExecutionRecoveryItem,
    RecoverableCeremonyWorkerOutcome, RecoverableCeremonyWorkerPort, RenewCeremonyStepLeaseUseCase,
    WorkerCapacityLimits,
};
use made_core::ports::NoopCeremonyEventSubscriber;
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyInstancePageLimit, CeremonyName,
    CeremonyVersion, DurationMs, ExecutionConnectorId, ExecutionRecoveryPageLimit, LeaseOwnerId,
    MaxParallel,
};
use made_core::DomainError;

const CHILD_MODE: &str = "MADE_DAEMON_CHILD";
const DEFINITION: &str = r#"
version: "1.0"
name: "multiprocess_worker"
description: "Real multiprocess worker claim fixture"
inputs: { required: [], optional: [] }
outputs: {}
states:
  - { id: OPEN, initial: true, terminal: false }
steps:
  - id: work
    state: OPEN
    handler: oci
    config: { connector: oci, provider: fixture }
roles:
  - id: WORKER
    allowed_actions: [work]
timeouts: { step_default: 30 }
retry_policies:
  default: { max_attempts: 3, backoff_seconds: 1 }
"#;

async fn definitions(path: &Path) -> Arc<InMemoryCeremonyDefinitionRepository> {
    let repository = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(path).unwrap()),
        repository.clone(),
    )
    .execute()
    .await
    .unwrap();
    repository
}

fn stream(store: &Arc<SqliteCeremonyStore>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

fn limits() -> WorkerCapacityLimits {
    WorkerCapacityLimits {
        global: CeremonyWorkerCapacity::new(2).unwrap(),
        per_root: CeremonyWorkerCapacity::new(2).unwrap(),
        per_connector: CeremonyWorkerCapacity::new(2).unwrap(),
        per_provider: CeremonyWorkerCapacity::new(2).unwrap(),
    }
}

#[tokio::test]
#[ignore = "subprocess helper"]
async fn daemon_child() {
    if std::env::var(CHILD_MODE).ok().as_deref() != Some("1") {
        return;
    }
    let database = PathBuf::from(std::env::var("MADE_CLAIM_DATABASE").unwrap());
    let capacity_directory = PathBuf::from(std::env::var("MADE_CLAIM_CAPACITY").unwrap());
    let definition_directory = PathBuf::from(std::env::var("MADE_CLAIM_DEFINITIONS").unwrap());
    let result = PathBuf::from(std::env::var("MADE_DAEMON_RESULT").unwrap());
    let owner = LeaseOwnerId::new(std::env::var("MADE_DAEMON_OWNER").unwrap()).unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let stream = stream(&store);
    let clock = Arc::new(SystemClock::new());
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions(&definition_directory).await,
        store.clone(),
    ));
    let deadlines = Arc::new(EnforceCeremonyDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = CeremonyWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(10).unwrap(),
    );
    let capacity = Arc::new(
        FileWorkerCapacityStore::new(capacity_directory, limits(), stream.clone()).unwrap(),
    );
    let claims = Arc::new(
        ClaimCeremonyWorkUseCase::new(
            store,
            stream.clone(),
            resolver.clone(),
            deadlines.clone(),
            Arc::new(StartCeremonyStepUseCase::new(
                resolver.clone(),
                stream.clone(),
                clock.clone(),
            )),
            clock.clone(),
            policy,
        )
        .with_shared_capacity(capacity.clone(), ExecutionConnectorId::new("oci").unwrap()),
    );
    let stop = CeremonyWorkerStopToken::new();
    let worker = Arc::new(RecordingWorker {
        accepted: Mutex::new(None),
        stop: stop.clone(),
    });
    let renewal = Arc::new(
        CeremonyWorkerRenewal::new(
            Arc::new(RenewCeremonyStepLeaseUseCase::new(stream, resolver, clock)),
            owner.clone(),
            DurationMs::from_millis(500),
            Duration::from_millis(100),
        )
        .unwrap()
        .with_shared_capacity(capacity),
    );
    let driver = Arc::new(
        CeremonyWorkerDriver::for_claims(deadlines, worker.clone(), policy, stop.clone())
            .with_renewal(renewal),
    );
    let input = ClaimCeremonyWorkInput::new(
        None,
        CeremonyInstancePageLimit::new(10).unwrap(),
        owner,
        DurationMs::from_millis(500),
        AuditActorKind::Service,
    );
    let daemon = Arc::new(CeremonyWorkerDaemon::new(
        Arc::new(CeremonyWorkerHost::new(claims, driver)),
        input,
        CeremonyWorkerHostPolicy::new(2, DurationMs::from_millis(10), DurationMs::from_millis(50))
            .unwrap(),
        stop,
    ));
    let run = daemon.run();
    tokio::pin!(run);
    tokio::select! {
        outcome = &mut run => { outcome.unwrap(); }
        () = tokio::time::sleep(Duration::from_secs(3)) => {
            daemon.request_stop();
            run.await.unwrap();
        }
    }
    let encoded = worker
        .accepted
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "none".to_owned());
    std::fs::write(result, encoded).unwrap();
}

fn spawn_child(root: &Path, result: &Path, owner: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg("daemon_child")
        .arg("--test-threads=1")
        .env(CHILD_MODE, "1")
        .env("MADE_CLAIM_DATABASE", root.join("ceremonies.sqlite3"))
        .env("MADE_CLAIM_CAPACITY", root.join("capacity"))
        .env("MADE_CLAIM_DEFINITIONS", root.join("definitions"))
        .env("MADE_DAEMON_RESULT", result)
        .env("MADE_DAEMON_OWNER", owner)
        .spawn()
        .unwrap()
}

#[tokio::test]
async fn three_daemons_compete_without_duplicate_effects() {
    let root = tempfile::tempdir_in("tmp").unwrap();
    let definition_directory = root.path().join("definitions");
    std::fs::create_dir_all(&definition_directory).unwrap();
    std::fs::write(definition_directory.join("worker.yaml"), DEFINITION).unwrap();
    let store =
        Arc::new(SqliteCeremonyStore::open(root.path().join("ceremonies.sqlite3")).unwrap());
    let stream = stream(&store);
    let clock = Arc::new(SystemClock::new());
    let repository = definitions(&definition_directory).await;
    let start = StartCeremonyUseCase::new(
        repository.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    );
    for index in 0..3 {
        start
            .execute(StartCeremonyInput::new(
                CeremonyId::new(format!("claim-{index}")).unwrap(),
                CeremonyName::new("multiprocess_worker").unwrap(),
                CeremonyVersion::v1(),
                CeremonyContext::empty(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();
    }

    let mut children = Vec::new();
    let mut results = Vec::new();
    for index in 0..3 {
        let result = root.path().join(format!("claim-result-{index}"));
        children.push(spawn_child(root.path(), &result, &format!("owner-{index}")));
        results.push(result);
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let accepted = results
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .filter(|value| value != "none")
        .collect::<Vec<_>>();
    assert_eq!(accepted.len(), 3, "all three durable operations must drain");
    let unique = accepted.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 3, "no ceremony effect may execute twice");
}

#[derive(Debug)]
struct RecordingWorker {
    accepted: Mutex<Option<String>>,
    stop: CeremonyWorkerStopToken,
}

#[async_trait]
impl RecoverableCeremonyWorkerPort for RecordingWorker {
    async fn execute_claim(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        tokio::time::sleep(Duration::from_millis(180)).await;
        *self.accepted.lock().unwrap() = Some(input.handler_request.instance_id().to_string());
        self.stop.request();
        Ok(RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
            made_core::value_objects::ExecutionOperationId::for_step(
                input.handler_request.instance_id(),
                input.handler_request.step_id(),
                input.state_visit,
                input.state_iteration,
                input.step_iteration,
            ),
        ))
    }

    async fn recover(
        &self,
        _item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        unreachable!("fixture has no durable execution intents")
    }
}
