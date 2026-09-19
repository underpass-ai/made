use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
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
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepInput,
    StartCeremonyStepUseCase, StartCeremonyUseCase,
};
use made_app::workers::{
    CeremonyWorkerCapacity, CeremonyWorkerDriver, CeremonyWorkerHost, CeremonyWorkerHostPolicy,
    CeremonyWorkerPolicy, CeremonyWorkerRenewal, CeremonyWorkerStopToken, ClaimCeremonyWorkInput,
    ClaimCeremonyWorkUseCase, ExecuteCeremonyOperationInput, ExecutionRecoveryInspectorPort,
    ExecutionRecoveryItem, ExecutionRecoveryItemsPage, RecoverableCeremonyWorkerOutcome,
    RecoverableCeremonyWorkerPort, RenewCeremonyStepLeaseUseCase, WorkerCapacityLimits,
    WorkerCapacityPort, WorkerCapacityRequest,
};
use made_core::ports::{ClockPort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyInstancePageLimit, CeremonyName,
    CeremonyVersion, DurationMs, ExecutionConnectorId, ExecutionOperation, ExecutionRecoveryCursor,
    ExecutionRecoveryPageLimit, ExecutionRequestBytes, LeaseOwnerId, MaxParallel, StateIteration,
    StateVisit, StepClaimFence, StepId, StepIteration,
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

fn single_capacity() -> WorkerCapacityLimits {
    WorkerCapacityLimits {
        global: CeremonyWorkerCapacity::new(1).unwrap(),
        per_root: CeremonyWorkerCapacity::new(1).unwrap(),
        per_connector: CeremonyWorkerCapacity::new(1).unwrap(),
        per_provider: CeremonyWorkerCapacity::new(1).unwrap(),
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

struct RecoveryFixture {
    _root: tempfile::TempDir,
    ceremony_id: CeremonyId,
    stream: Arc<SessionStream>,
    clock: Arc<ControlledClock>,
    resolver: Arc<ResolveCeremonyDefinitionUseCase>,
    deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    capacity: Arc<FileWorkerCapacityStore>,
    owner: LeaseOwnerId,
    claim: ExecuteCeremonyOperationInput,
    operation: ExecutionOperation,
    item: ExecutionRecoveryItem,
}

#[derive(Debug)]
struct ControlledClock {
    millis: AtomicI64,
}

impl ControlledClock {
    fn new() -> Self {
        Self {
            millis: AtomicI64::new(1_000_000),
        }
    }

    fn advance(&self, duration: Duration) {
        let millis = i64::try_from(duration.as_millis()).expect("test duration fits in i64");
        self.millis.fetch_add(millis, Ordering::SeqCst);
    }
}

impl ClockPort for ControlledClock {
    fn now(&self) -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp_nanos(
            i128::from(self.millis.load(Ordering::SeqCst)) * 1_000_000,
        )
        .expect("test clock timestamp is valid")
    }
}

fn recovery_operation(
    claim: &ExecuteCeremonyOperationInput,
) -> (ExecutionOperation, ExecutionRecoveryItem) {
    let operation = ExecutionOperation::new(
        claim.handler_request.instance_id().clone(),
        claim.handler_request.step_id().clone(),
        claim.state_visit,
        claim.state_iteration,
        claim.step_iteration,
        ExecutionRequestBytes::new(b"recovery lookup".to_vec()).unwrap(),
    );
    let item = ExecutionRecoveryItem::new(
        operation.clone(),
        Vec::new(),
        None,
        Some(claim.claim_fence.clone()),
    );
    (operation, item)
}

async fn recovery_fixture() -> RecoveryFixture {
    std::fs::create_dir_all("tmp").unwrap();
    let root = tempfile::tempdir_in("tmp").unwrap();
    let definition_directory = root.path().join("definitions");
    std::fs::create_dir_all(&definition_directory).unwrap();
    std::fs::write(definition_directory.join("worker.yaml"), DEFINITION).unwrap();
    let store =
        Arc::new(SqliteCeremonyStore::open(root.path().join("ceremonies.sqlite3")).unwrap());
    let stream = stream(&store);
    let clock = Arc::new(ControlledClock::new());
    let repository = definitions(&definition_directory).await;
    let start = StartCeremonyUseCase::new(
        repository.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    );
    let ceremony_id = CeremonyId::new("recovery-renewal").unwrap();
    start
        .execute(StartCeremonyInput::new(
            ceremony_id.clone(),
            CeremonyName::new("multiprocess_worker").unwrap(),
            CeremonyVersion::v1(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let policy = CeremonyWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(10).unwrap(),
    );
    let deadlines = Arc::new(EnforceCeremonyDeadlinesUseCase::new(
        Arc::new(ResolveCeremonyDefinitionUseCase::new(
            repository.clone(),
            store.clone(),
        )),
        stream.clone(),
        clock.clone(),
    ));
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        repository,
        store.clone(),
    ));
    let capacity = Arc::new(
        FileWorkerCapacityStore::new(
            root.path().join("capacity"),
            single_capacity(),
            stream.clone(),
        )
        .unwrap(),
    );
    let owner = LeaseOwnerId::new("recovery-owner").unwrap();
    let claims = ClaimCeremonyWorkUseCase::new(
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
    .with_shared_capacity(capacity.clone(), ExecutionConnectorId::new("oci").unwrap());
    let claim = claims
        .execute(ClaimCeremonyWorkInput::new(
            None,
            CeremonyInstancePageLimit::new(10).unwrap(),
            owner.clone(),
            DurationMs::from_millis(120),
            AuditActorKind::Service,
        ))
        .await
        .unwrap()
        .claims()
        .first()
        .cloned()
        .expect("fixture claim must be admitted");
    let (operation, item) = recovery_operation(&claim);
    RecoveryFixture {
        _root: root,
        ceremony_id,
        stream,
        clock,
        resolver,
        deadlines,
        capacity,
        owner,
        claim,
        operation,
        item,
    }
}

fn spawn_competitor(
    capacity: Arc<FileWorkerCapacityStore>,
    clock: Arc<ControlledClock>,
    root_id: CeremonyId,
) -> tokio::task::JoinHandle<bool> {
    tokio::spawn(async move {
        let now = clock.now();
        let competitor_step = StepId::new("work").unwrap();
        capacity
            .reserve(
                &WorkerCapacityRequest {
                    operation_id: made_core::value_objects::ExecutionOperationId::for_step(
                        &CeremonyId::new("competitor").unwrap(),
                        &competitor_step,
                        StateVisit::FIRST,
                        StateIteration::FIRST,
                        StepIteration::FIRST,
                    ),
                    ceremony_id: CeremonyId::new("competitor").unwrap(),
                    step_id: competitor_step,
                    root_id,
                    connector_id: ExecutionConnectorId::new("oci").unwrap(),
                    provider_id: ExecutionConnectorId::new("fixture").unwrap(),
                    owner_id: LeaseOwnerId::new("competitor-owner").unwrap(),
                    pending_until: now + time::Duration::seconds(1),
                },
                now,
            )
            .await
            .unwrap()
    })
}

async fn assert_recovery_cancels_after_capacity_loss(
    stream: Arc<SessionStream>,
    resolver: Arc<ResolveCeremonyDefinitionUseCase>,
    clock: Arc<ControlledClock>,
    owner: LeaseOwnerId,
    claim_fence: StepClaimFence,
    evidence: ExecutionRecoveryItem,
) {
    let late_worker = Arc::new(SlowRecoveryWorker {
        calls: Arc::new(AtomicUsize::new(0)),
        observed: Arc::new(Mutex::new(None)),
        started: Arc::new(tokio::sync::Notify::new()),
    });
    let stale_capacity = Arc::new(FailingRenewalCapacity {
        renewals: AtomicUsize::new(0),
    });
    let stale_renewal = CeremonyWorkerRenewal::new(
        Arc::new(RenewCeremonyStepLeaseUseCase::new(
            stream,
            resolver,
            clock.clone(),
        )),
        owner,
        DurationMs::from_millis(1_000),
        Duration::from_millis(100),
    )
    .unwrap()
    .with_shared_capacity(stale_capacity);
    let stale_recovery = stale_renewal.recover(late_worker.as_ref(), evidence.clone());
    tokio::pin!(stale_recovery);
    tokio::select! {
        () = late_worker.started.notified() => {}
        result = &mut stale_recovery => panic!("recovery ended before lookup started: {result:?}"),
    }
    clock.advance(Duration::from_millis(100));
    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    let stale_result = stale_recovery.await;
    assert!(
        stale_result.is_err(),
        "replaced capacity must cancel recovery"
    );
    assert_eq!(late_worker.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        evidence.current_claim_fence(),
        Some(&claim_fence),
        "cancelled recovery must retain its durable evidence"
    );
}

async fn reclaim_expired_claim(
    stream: Arc<SessionStream>,
    resolver: Arc<ResolveCeremonyDefinitionUseCase>,
    clock: Arc<ControlledClock>,
    capacity: Arc<FileWorkerCapacityStore>,
    claim: &ExecuteCeremonyOperationInput,
) -> StepClaimFence {
    clock.advance(Duration::from_millis(121));
    let owner = LeaseOwnerId::new("competitor-owner").unwrap();
    let operation = made_core::value_objects::ExecutionOperationId::for_step(
        claim.handler_request.instance_id(),
        claim.handler_request.step_id(),
        claim.state_visit,
        claim.state_iteration,
        claim.step_iteration,
    );
    let now = clock.now();
    assert!(capacity
        .reserve(
            &WorkerCapacityRequest {
                operation_id: operation.clone(),
                ceremony_id: claim.handler_request.instance_id().clone(),
                step_id: claim.handler_request.step_id().clone(),
                root_id: claim.handler_request.instance_id().clone(),
                connector_id: ExecutionConnectorId::new("oci").unwrap(),
                provider_id: ExecutionConnectorId::new("fixture").unwrap(),
                owner_id: owner.clone(),
                pending_until: now + time::Duration::seconds(1),
            },
            now,
        )
        .await
        .unwrap());
    let competitor = StartCeremonyStepUseCase::new(resolver, stream, clock)
        .execute(StartCeremonyStepInput::new(
            claim.handler_request.instance_id().clone(),
            made_core::value_objects::RoleId::new("WORKER").unwrap(),
            AuditActorKind::Service,
            claim.handler_request.step_id().clone(),
            owner.clone(),
            made_core::value_objects::IdempotencyKey::new("competitor-reclaim").unwrap(),
            DurationMs::from_millis(120),
        ))
        .await
        .unwrap();
    let fence = competitor.claim_fence().clone();
    capacity.bind(&operation, &owner, &fence).await.unwrap();
    fence
}

#[tokio::test]
async fn stale_recovery_owner_loses_journal_authority_to_competing_claim() {
    tokio::time::pause();
    let RecoveryFixture {
        _root,
        stream,
        clock,
        resolver,
        deadlines,
        capacity,
        owner,
        claim,
        item,
        ..
    } = recovery_fixture().await;
    let worker = Arc::new(SlowRecoveryWorker {
        calls: Arc::new(AtomicUsize::new(0)),
        observed: Arc::new(Mutex::new(None)),
        started: Arc::new(tokio::sync::Notify::new()),
    });
    let renewal = CeremonyWorkerRenewal::new(
        Arc::new(RenewCeremonyStepLeaseUseCase::new(
            stream.clone(),
            resolver.clone(),
            clock.clone(),
        )),
        owner,
        DurationMs::from_millis(120),
        Duration::from_millis(30),
    )
    .unwrap();
    let driver = CeremonyWorkerDriver::new(
        Arc::new(SingleRecoveryInspector { item }),
        deadlines,
        worker.clone(),
        CeremonyWorkerPolicy::new(
            MaxParallel::new(1).unwrap(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
        ),
        CeremonyWorkerStopToken::new(),
    )
    .with_renewal(Arc::new(renewal));
    let recovery = driver.recover_page(None);
    tokio::pin!(recovery);
    tokio::select! {
        () = worker.started.notified() => {}
        result = &mut recovery => panic!("recovery ended before lookup started: {result:?}"),
    }

    // A's short lease is now expired. B reclaims the capacity and journal
    // operation root with a new fence.
    let competitor_fence = reclaim_expired_claim(
        stream.clone(),
        resolver.clone(),
        clock.clone(),
        capacity,
        &claim,
    )
    .await;
    assert_ne!(&competitor_fence, &claim.claim_fence);

    // Let A's already-scheduled heartbeat observe B's real journal fence.
    tokio::time::advance(Duration::from_millis(30)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let outcome = recovery.await.unwrap();
    assert!(outcome.outcomes().is_empty());
    assert_eq!(outcome.failures().len(), 1);
    assert!(matches!(
        outcome.failures()[0].error(),
        DomainError::InvariantViolated { reason }
            if *reason == "step completion claim fence does not match the current claim"
    ));
    assert_eq!(worker.calls.load(Ordering::SeqCst), 0);
    assert!(worker.observed.lock().unwrap().is_none());
}

#[tokio::test]
async fn recovery_renews_original_fence_and_capacity_during_slow_lookup() {
    tokio::time::pause();
    let RecoveryFixture {
        _root,
        ceremony_id,
        stream,
        clock,
        resolver,
        deadlines,
        capacity,
        owner,
        claim,
        operation,
        item,
    } = recovery_fixture().await;
    let evidence = item.clone();
    let worker = Arc::new(SlowRecoveryWorker {
        calls: Arc::new(AtomicUsize::new(0)),
        observed: Arc::new(Mutex::new(None)),
        started: Arc::new(tokio::sync::Notify::new()),
    });
    let renewal = CeremonyWorkerRenewal::new(
        Arc::new(RenewCeremonyStepLeaseUseCase::new(
            stream.clone(),
            resolver.clone(),
            clock.clone(),
        )),
        owner.clone(),
        DurationMs::from_millis(1_000),
        Duration::from_millis(100),
    )
    .unwrap()
    .with_shared_capacity(capacity.clone());
    let stop = CeremonyWorkerStopToken::new();
    let driver = CeremonyWorkerDriver::new(
        Arc::new(SingleRecoveryInspector { item }),
        deadlines,
        worker.clone(),
        CeremonyWorkerPolicy::new(
            MaxParallel::new(1).unwrap(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
        ),
        stop,
    )
    .with_renewal(Arc::new(renewal));
    let recovery = driver.recover_page(None);
    tokio::pin!(recovery);
    tokio::select! {
        () = worker.started.notified() => {}
        result = &mut recovery => panic!("recovery ended before lookup started: {result:?}"),
    }
    let competitor = spawn_competitor(capacity.clone(), clock.clone(), ceremony_id);
    assert!(
        !competitor.await.unwrap(),
        "competitor must not steal live capacity"
    );
    for _ in 0..4 {
        clock.advance(Duration::from_millis(100));
        tokio::time::advance(Duration::from_millis(100)).await;
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }
    let outcome = recovery.await.unwrap();
    assert!(outcome.failures().is_empty());
    assert!(matches!(
        outcome.outcomes(),
        [RecoverableCeremonyWorkerOutcome::ReconciliationRequired(id)] if id == operation.operation_id()
    ));
    assert_eq!(worker.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        worker.observed.lock().unwrap().as_ref(),
        Some(&(operation.operation_id().clone(), claim.claim_fence.clone()))
    );

    assert_recovery_cancels_after_capacity_loss(
        stream,
        resolver,
        clock,
        owner,
        claim.claim_fence.clone(),
        evidence,
    )
    .await;
}

#[tokio::test]
async fn three_daemons_compete_without_duplicate_effects() {
    std::fs::create_dir_all("tmp").unwrap();
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
struct SlowRecoveryWorker {
    calls: Arc<AtomicUsize>,
    observed: Arc<
        Mutex<
            Option<(
                made_core::value_objects::ExecutionOperationId,
                StepClaimFence,
            )>,
        >,
    >,
    started: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl RecoverableCeremonyWorkerPort for SlowRecoveryWorker {
    async fn execute_claim(
        &self,
        _input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        unreachable!("fixture only exercises recovery")
    }

    async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        self.started.notify_one();
        tokio::time::sleep(Duration::from_millis(360)).await;
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.observed.lock().unwrap() = Some((
            item.operation().operation_id().clone(),
            item.current_claim_fence().cloned().unwrap(),
        ));
        Ok(RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
            item.operation().operation_id().clone(),
        ))
    }
}

#[derive(Debug)]
struct SingleRecoveryInspector {
    item: ExecutionRecoveryItem,
}

#[async_trait]
impl ExecutionRecoveryInspectorPort for SingleRecoveryInspector {
    async fn inspect(
        &self,
        _after: Option<&ExecutionRecoveryCursor>,
        _limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
        Ok(ExecutionRecoveryItemsPage::new(
            vec![self.item.clone()],
            None,
        ))
    }
}

#[derive(Debug)]
struct FailingRenewalCapacity {
    renewals: AtomicUsize,
}

#[async_trait]
impl WorkerCapacityPort for FailingRenewalCapacity {
    async fn reserve(
        &self,
        _request: &WorkerCapacityRequest,
        _now: time::OffsetDateTime,
    ) -> Result<bool, DomainError> {
        Ok(false)
    }

    async fn bind(
        &self,
        _operation: &made_core::value_objects::ExecutionOperationId,
        _owner: &LeaseOwnerId,
        _fence: &StepClaimFence,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn lock_renewal(
        &self,
        _operation: &made_core::value_objects::ExecutionOperationId,
        _owner: &LeaseOwnerId,
        _fence: &StepClaimFence,
    ) -> Result<Box<dyn made_app::workers::WorkerCapacityRenewalGuard>, DomainError> {
        if self.renewals.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(Box::new(()))
        } else {
            Err(DomainError::Conflict {
                what: "worker_capacity_fence",
            })
        }
    }

    async fn release(
        &self,
        _operation: &made_core::value_objects::ExecutionOperationId,
        _owner: &LeaseOwnerId,
    ) -> Result<(), DomainError> {
        Ok(())
    }
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
