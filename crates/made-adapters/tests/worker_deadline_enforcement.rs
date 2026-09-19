use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
    InMemoryCeremonyEventStore,
};
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceCeremonyDeadlinesUseCase, MountCeremonyDefinitionsUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepInput,
    StartCeremonyStepUseCase, StartCeremonyUseCase,
};
use made_app::workers::{
    CeremonyWorkerDriver, CeremonyWorkerHost, CeremonyWorkerPolicy, CeremonyWorkerStopToken,
    ClaimCeremonyWorkInput, ClaimCeremonyWorkUseCase, ExecuteCeremonyOperationInput,
    ExecutionRecoveryItem, RecoverableCeremonyWorkerOutcome, RecoverableCeremonyWorkerPort,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerRequest, ClockPort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, DurationMs,
    ExecutionOperationId, ExecutionRecoveryPageLimit, IdempotencyKey, LeaseOwnerId, MaxParallel,
    RoleId, StepHandlerConfig, StepHandlerKind, StepId, StepStatus,
};
use time::OffsetDateTime;

const DEFINITION: &str = r#"
version: "1.0"
name: "deadline_worker"
description: "Deadline enforcement before worker admission"
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
    allowed_actions: [work]
timeouts:
  step_default: 1
retry_policies:
  default:
    max_attempts: 2
    backoff_seconds: 1
"#;

#[derive(Debug)]
struct MutableClock(RwLock<OffsetDateTime>);

impl MutableClock {
    fn new(now: OffsetDateTime) -> Self {
        Self(RwLock::new(now))
    }

    fn set(&self, now: OffsetDateTime) {
        *self.0.write().unwrap() = now;
    }
}

impl ClockPort for MutableClock {
    fn now(&self) -> OffsetDateTime {
        *self.0.read().unwrap()
    }
}

#[derive(Debug, Default)]
struct CountingWorker(AtomicUsize);

#[async_trait]
impl RecoverableCeremonyWorkerPort for CountingWorker {
    async fn execute_claim(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
            ExecutionOperationId::for_step(
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
        unreachable!("this test only admits a fresh accepted claim")
    }
}

#[tokio::test]
async fn expired_step_is_sealed_before_the_driver_can_admit_external_work() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    std::fs::write(directory.path().join("deadline_worker.yaml"), DEFINITION).unwrap();
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(directory.path()).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let events = Arc::new(InMemoryCeremonyEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events,
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    let ceremony_id = CeremonyId::new("expired-worker").unwrap();
    StartCeremonyUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartCeremonyInput::new(
        ceremony_id.clone(),
        CeremonyName::new("deadline_worker").unwrap(),
        CeremonyVersion::v1(),
        CeremonyContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryCeremonyDefinitionPublications::new()),
    ));
    let step_id = StepId::new("work").unwrap();
    let claim = StartCeremonyStepUseCase::new(resolver.clone(), stream.clone(), clock.clone())
        .execute(StartCeremonyStepInput::new(
            ceremony_id.clone(),
            RoleId::new("WORKER").unwrap(),
            AuditActorKind::Agent,
            step_id.clone(),
            LeaseOwnerId::new("worker-1").unwrap(),
            IdempotencyKey::new("expired-worker-claim").unwrap(),
            DurationMs::from_millis(60_000),
        ))
        .await
        .unwrap();
    let record = claim.instance().step_record(&step_id).unwrap();
    let input = ExecuteCeremonyOperationInput {
        handler_request: CeremonyStepHandlerRequest::new(
            ceremony_id.clone(),
            CeremonyName::new("deadline_worker").unwrap(),
            CeremonyVersion::v1(),
            claim.instance().current_state().clone(),
            step_id.clone(),
            StepHandlerKind::new("fixture").unwrap(),
            StepHandlerConfig::empty(),
            claim.instance().context().clone(),
            claim.attempt(),
        )
        .with_role(RoleId::new("WORKER").unwrap()),
        state_visit: record.state_visit(),
        state_iteration: record.state_iteration(),
        step_iteration: record.iteration(),
        claim_fence: claim.claim_fence().clone(),
        actor_kind: AuditActorKind::Agent,
    };
    clock.set(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(2));
    let worker = Arc::new(CountingWorker::default());
    let driver = CeremonyWorkerDriver::for_claims(
        Arc::new(EnforceCeremonyDeadlinesUseCase::new(
            resolver,
            stream.clone(),
            clock,
        )),
        worker.clone(),
        CeremonyWorkerPolicy::new(
            MaxParallel::new(1).unwrap(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
        ),
        CeremonyWorkerStopToken::new(),
    );

    let outcome = driver.execute_claims(vec![input]).await.unwrap();

    assert!(outcome.outcomes().is_empty());
    assert_eq!(worker.0.load(Ordering::SeqCst), 0);
    let expired = stream.load(&ceremony_id).await.unwrap();
    assert_eq!(
        expired.instance.step_record(&step_id).unwrap().status(),
        StepStatus::Failed
    );
}

#[tokio::test]
async fn reference_host_discovers_claims_and_drains_one_public_step() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    std::fs::write(directory.path().join("deadline_worker.yaml"), DEFINITION).unwrap();
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(directory.path()).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let events = Arc::new(InMemoryCeremonyEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events,
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    let start = StartCeremonyUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    );
    for ceremony_id in ["discoverable-worker-1", "discoverable-worker-2"] {
        start
            .execute(StartCeremonyInput::new(
                CeremonyId::new(ceremony_id).unwrap(),
                CeremonyName::new("deadline_worker").unwrap(),
                CeremonyVersion::v1(),
                CeremonyContext::empty(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();
    }
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryCeremonyDefinitionPublications::new()),
    ));
    let deadlines = Arc::new(EnforceCeremonyDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = CeremonyWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(1).unwrap(),
    );
    let claims = Arc::new(ClaimCeremonyWorkUseCase::new(
        stream.clone(),
        resolver.clone(),
        deadlines.clone(),
        Arc::new(StartCeremonyStepUseCase::new(
            resolver,
            stream,
            clock.clone(),
        )),
        clock,
        policy,
    ));
    let worker = Arc::new(CountingWorker::default());
    let driver = Arc::new(CeremonyWorkerDriver::for_claims(
        deadlines,
        worker.clone(),
        policy,
        CeremonyWorkerStopToken::new(),
    ));
    let host = CeremonyWorkerHost::new(claims, driver);

    let first = host
        .run_claim_page(ClaimCeremonyWorkInput::new(
            None,
            ExecutionRecoveryPageLimit::new(1).unwrap(),
            LeaseOwnerId::new("reference-host").unwrap(),
            DurationMs::from_millis(60_000),
            AuditActorKind::Engine,
        ))
        .await
        .unwrap();
    let second = host
        .run_claim_page(ClaimCeremonyWorkInput::new(
            first.next_cursor().cloned(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
            LeaseOwnerId::new("reference-host").unwrap(),
            DurationMs::from_millis(60_000),
            AuditActorKind::Engine,
        ))
        .await
        .unwrap();

    assert!(first.claim_failures().is_empty());
    assert!(first.batch().failures().is_empty());
    assert_eq!(first.batch().outcomes().len(), 1);
    assert!(first.next_cursor().is_some());
    assert!(second.claim_failures().is_empty());
    assert_eq!(second.batch().outcomes().len(), 1);
    assert_eq!(worker.0.load(Ordering::SeqCst), 2);
}
