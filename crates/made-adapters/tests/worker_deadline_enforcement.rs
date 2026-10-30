use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryBudgetLedgerStore, InMemoryProtocolDefinitionPublications,
    InMemoryProtocolDefinitionRepository, InMemoryProtocolEventStore,
};
use made_adapters::yaml::FileSystemProtocolDefinitionSource;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedProtocolInput,
    StartBudgetedProtocolUseCase,
};
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceProtocolDeadlinesUseCase, MountProtocolDefinitionsUseCase,
    ResolveProtocolDefinitionUseCase, StartProtocolInput, StartProtocolStepInput,
    StartProtocolStepUseCase, StartProtocolUseCase,
};
use made_app::workers::{
    ClaimProtocolWorkInput, ClaimProtocolWorkUseCase, ExecuteProtocolOperationInput,
    ExecutionRecoveryItem, ProtocolWorkerDriver, ProtocolWorkerHost, ProtocolWorkerPolicy,
    ProtocolWorkerStopToken, RecoverableProtocolWorkerOutcome, RecoverableProtocolWorkerPort,
};
use made_core::error::DomainError;
use made_core::ports::{
    BudgetReservationPlannerPort, ClockPort, NoopProtocolEventSubscriber,
    ProtocolDefinitionPublicationPort, ProtocolDefinitionRepositoryPort,
    ProtocolStepHandlerRequest,
};
use made_core::value_objects::{
    AuditActorKind, BudgetLimits, BudgetMeasurement, BudgetQuantities, BudgetReservationEstimate,
    BudgetReservationRequest, BudgetTokenCount, CostMicros, CurrencyCode, DurationMs,
    ExecutionDuration, ExecutionOperationId, ExecutionRecoveryPageLimit, IdempotencyKey,
    LeaseOwnerId, MaxParallel, ProtocolContext, ProtocolId, ProtocolInstancePageLimit,
    ProtocolName, ProtocolVersion, RoleId, StepHandlerConfig, StepHandlerKind, StepId, StepStatus,
    ToolCallCount,
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

#[derive(Debug)]
struct UnknownTokenPlanner;

#[async_trait]
impl BudgetReservationPlannerPort for UnknownTokenPlanner {
    async fn estimate(
        &self,
        _request: &BudgetReservationRequest,
    ) -> Result<BudgetReservationEstimate, DomainError> {
        Ok(BudgetReservationEstimate::new(
            BudgetMeasurement::Estimated(ExecutionDuration::from_micros(1_000)),
            BudgetMeasurement::Unknown,
            BudgetMeasurement::Estimated(CostMicros::new(1)),
            BudgetMeasurement::Estimated(ToolCallCount::new(1)),
        ))
    }
}

#[async_trait]
impl RecoverableProtocolWorkerPort for CountingWorker {
    async fn execute_claim(
        &self,
        input: ExecuteProtocolOperationInput,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(RecoverableProtocolWorkerOutcome::ReconciliationRequired(
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
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        unreachable!("this test only admits a fresh accepted claim")
    }
}

async fn mounted_published_definition() -> (
    Arc<InMemoryProtocolDefinitionRepository>,
    Arc<InMemoryProtocolDefinitionPublications>,
) {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    std::fs::write(directory.path().join("deadline_worker.yaml"), DEFINITION).unwrap();
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    MountProtocolDefinitionsUseCase::new(
        Arc::new(FileSystemProtocolDefinitionSource::from_directory(directory.path()).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let name = ProtocolName::new("deadline_worker").unwrap();
    let version = ProtocolVersion::v1();
    let definition = definitions.get(&name, &version).await.unwrap();
    let publications = Arc::new(InMemoryProtocolDefinitionPublications::new());
    publications
        .publish(made_core::entities::PublishedProtocolDefinition::seal(definition).unwrap())
        .await
        .unwrap();
    (definitions, publications)
}

async fn start_budgeted_worker_protocol(
    publications: Arc<InMemoryProtocolDefinitionPublications>,
    stream: Arc<SessionStream>,
    clock: Arc<MutableClock>,
    budgets: BudgetLedgerService,
    protocol_id: ProtocolId,
) {
    StartBudgetedProtocolUseCase::new(
        publications,
        stream,
        clock,
        Arc::new(ForgetfulMemory::new()),
        budgets,
    )
    .execute(StartBudgetedProtocolInput::new(
        StartProtocolInput::new(
            protocol_id,
            ProtocolName::new("deadline_worker").unwrap(),
            ProtocolVersion::v1(),
            ProtocolContext::empty(),
            "operator",
            AuditActorKind::Service,
        ),
        BudgetLimits::new(
            BudgetQuantities::new(
                ExecutionDuration::from_micros(10_000),
                BudgetTokenCount::new(100),
                CostMicros::new(100),
                ToolCallCount::new(10),
            ),
            Some(CurrencyCode::new("EUR").unwrap()),
        )
        .unwrap(),
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn expired_step_is_sealed_before_the_driver_can_admit_external_work() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    std::fs::write(directory.path().join("deadline_worker.yaml"), DEFINITION).unwrap();
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    MountProtocolDefinitionsUseCase::new(
        Arc::new(FileSystemProtocolDefinitionSource::from_directory(directory.path()).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let events = Arc::new(InMemoryProtocolEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    let protocol_id = ProtocolId::new("expired-worker").unwrap();
    StartProtocolUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartProtocolInput::new(
        protocol_id.clone(),
        ProtocolName::new("deadline_worker").unwrap(),
        ProtocolVersion::v1(),
        ProtocolContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryProtocolDefinitionPublications::new()),
    ));
    let step_id = StepId::new("work").unwrap();
    let claim = StartProtocolStepUseCase::new(resolver.clone(), stream.clone(), clock.clone())
        .execute(StartProtocolStepInput::new(
            protocol_id.clone(),
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
    let input = ExecuteProtocolOperationInput {
        handler_request: ProtocolStepHandlerRequest::new(
            protocol_id.clone(),
            ProtocolName::new("deadline_worker").unwrap(),
            ProtocolVersion::v1(),
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
    let driver = ProtocolWorkerDriver::for_claims(
        Arc::new(EnforceProtocolDeadlinesUseCase::new(
            resolver,
            stream.clone(),
            clock,
        )),
        worker.clone(),
        ProtocolWorkerPolicy::new(
            MaxParallel::new(1).unwrap(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
        ),
        ProtocolWorkerStopToken::new(),
    );

    let outcome = driver.execute_claims(vec![input]).await.unwrap();

    assert!(outcome.outcomes().is_empty());
    assert_eq!(worker.0.load(Ordering::SeqCst), 0);
    let expired = stream.load(&protocol_id).await.unwrap();
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
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    MountProtocolDefinitionsUseCase::new(
        Arc::new(FileSystemProtocolDefinitionSource::from_directory(directory.path()).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let events = Arc::new(InMemoryProtocolEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    let start = StartProtocolUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    );
    for protocol_id in ["discoverable-worker-1", "discoverable-worker-2"] {
        start
            .execute(StartProtocolInput::new(
                ProtocolId::new(protocol_id).unwrap(),
                ProtocolName::new("deadline_worker").unwrap(),
                ProtocolVersion::v1(),
                ProtocolContext::empty(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();
    }
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryProtocolDefinitionPublications::new()),
    ));
    let deadlines = Arc::new(EnforceProtocolDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = ProtocolWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(1).unwrap(),
    );
    let claims = Arc::new(ClaimProtocolWorkUseCase::new(
        events,
        stream.clone(),
        resolver.clone(),
        deadlines.clone(),
        Arc::new(StartProtocolStepUseCase::new(
            resolver,
            stream,
            clock.clone(),
        )),
        clock,
        policy,
    ));
    let worker = Arc::new(CountingWorker::default());
    let driver = Arc::new(ProtocolWorkerDriver::for_claims(
        deadlines,
        worker.clone(),
        policy,
        ProtocolWorkerStopToken::new(),
    ));
    let host = ProtocolWorkerHost::new(claims, driver);

    let first = host
        .run_claim_page(ClaimProtocolWorkInput::new(
            None,
            ProtocolInstancePageLimit::new(1).unwrap(),
            LeaseOwnerId::new("reference-host").unwrap(),
            DurationMs::from_millis(60_000),
            AuditActorKind::Engine,
        ))
        .await
        .unwrap();
    let second = host
        .run_claim_page(ClaimProtocolWorkInput::new(
            first.next_cursor().cloned(),
            ProtocolInstancePageLimit::new(1).unwrap(),
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

#[tokio::test]
async fn unknown_limited_estimate_stops_before_claim_intent_or_effect() {
    let (definitions, publications) = mounted_published_definition().await;
    let events = Arc::new(InMemoryProtocolEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    let budgets =
        BudgetLedgerService::new(Arc::new(InMemoryBudgetLedgerStore::new()), clock.clone());
    let protocol_id = ProtocolId::new("budgeted-unknown").unwrap();
    start_budgeted_worker_protocol(
        publications.clone(),
        stream.clone(),
        clock.clone(),
        budgets.clone(),
        protocol_id.clone(),
    )
    .await;
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        publications,
    ));
    let deadlines = Arc::new(EnforceProtocolDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = ProtocolWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(1).unwrap(),
    );
    let claims = Arc::new(
        ClaimProtocolWorkUseCase::new(
            events,
            stream.clone(),
            resolver.clone(),
            deadlines.clone(),
            Arc::new(StartProtocolStepUseCase::new(
                resolver.clone(),
                stream.clone(),
                clock.clone(),
            )),
            clock.clone(),
            policy,
        )
        .with_budget_admission(
            Arc::new(BudgetedStepClaimUseCase::new(
                resolver,
                stream.clone(),
                clock,
                budgets.clone(),
            )),
            Arc::new(UnknownTokenPlanner),
        ),
    );
    let worker = Arc::new(CountingWorker::default());
    let host = ProtocolWorkerHost::new(
        claims,
        Arc::new(ProtocolWorkerDriver::for_claims(
            deadlines,
            worker.clone(),
            policy,
            ProtocolWorkerStopToken::new(),
        )),
    );

    let outcome = host
        .run_claim_page(ClaimProtocolWorkInput::new(
            None,
            ProtocolInstancePageLimit::new(1).unwrap(),
            LeaseOwnerId::new("budget-host").unwrap(),
            DurationMs::from_millis(60_000),
            AuditActorKind::Engine,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.claim_failures().len(), 1);
    assert!(outcome.batch().outcomes().is_empty());
    assert_eq!(worker.0.load(Ordering::SeqCst), 0);
    let instance = stream.load(&protocol_id).await.unwrap().instance;
    assert_eq!(
        instance
            .step_record(&StepId::new("work").unwrap())
            .unwrap()
            .status(),
        StepStatus::Pending
    );
    let account = instance.budget_account_id().unwrap();
    assert_eq!(
        budgets
            .report(account)
            .await
            .unwrap()
            .reserved()
            .tokens()
            .value(),
        0
    );
}

async fn started_worker_protocol(
    protocol: &str,
) -> (
    Arc<ClaimProtocolWorkUseCase>,
    Arc<SessionStream>,
    Arc<MutableClock>,
) {
    let (definitions, publications) = mounted_published_definition().await;
    let events = Arc::new(InMemoryProtocolEventStore::new());
    let stream = Arc::new(SessionStream::new(
        events.clone(),
        events.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let clock = Arc::new(MutableClock::new(OffsetDateTime::UNIX_EPOCH));
    StartProtocolUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartProtocolInput::new(
        ProtocolId::new(protocol).unwrap(),
        ProtocolName::new("deadline_worker").unwrap(),
        ProtocolVersion::v1(),
        ProtocolContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        publications,
    ));
    let claims = Arc::new(ClaimProtocolWorkUseCase::new(
        events,
        stream.clone(),
        resolver.clone(),
        Arc::new(EnforceProtocolDeadlinesUseCase::new(
            resolver.clone(),
            stream.clone(),
            clock.clone(),
        )),
        Arc::new(StartProtocolStepUseCase::new(
            resolver,
            stream.clone(),
            clock.clone(),
        )),
        clock.clone(),
        ProtocolWorkerPolicy::new(
            MaxParallel::new(1).unwrap(),
            ExecutionRecoveryPageLimit::new(1).unwrap(),
        ),
    ));
    (claims, stream, clock)
}

fn claim_as(owner: &str, lease_ms: u64) -> ClaimProtocolWorkInput {
    ClaimProtocolWorkInput::new(
        None,
        ProtocolInstancePageLimit::new(1).unwrap(),
        LeaseOwnerId::new(owner).unwrap(),
        DurationMs::from_millis(lease_ms),
        AuditActorKind::Engine,
    )
}

// Regression for #254: the worker derived the claim idempotency key from the
// record's current attempt, so the claim that replaces a dead owner's expired
// lease reused the dead owner's key and was refused as AlreadyExists.
#[tokio::test]
async fn an_expired_dead_owner_claim_is_reclaimed_by_another_owner() {
    let (claims, stream, clock) = started_worker_protocol("dead-owner").await;
    let first = claims.execute(claim_as("dead-owner", 500)).await.unwrap();
    assert_eq!(first.claims().len(), 1, "failures: {:?}", first.failures());

    // The lease (500 ms) has expired, the step deadline (1 s) has not.
    clock.set(OffsetDateTime::UNIX_EPOCH + time::Duration::milliseconds(600));
    let second = claims.execute(claim_as("replacement", 500)).await.unwrap();

    assert!(second.failures().is_empty(), "{:?}", second.failures());
    assert_eq!(second.claims().len(), 1);
    assert_eq!(second.claims()[0].handler_request.attempt().get(), 2);
    let record = stream
        .load(&ProtocolId::new("dead-owner").unwrap())
        .await
        .unwrap()
        .instance
        .step_record(&StepId::new("work").unwrap())
        .cloned()
        .unwrap();
    assert_eq!(record.lease().unwrap().owner_id().as_str(), "replacement");
}

// Same key collision on the retry path: a step whose first attempt timed out
// must be claimable for its second attempt once the backoff has elapsed.
#[tokio::test]
async fn a_timed_out_attempt_is_retried_by_the_worker_after_backoff() {
    let (claims, stream, clock) = started_worker_protocol("timed-out").await;
    let first = claims
        .execute(claim_as("first-owner", 60_000))
        .await
        .unwrap();
    assert_eq!(first.claims().len(), 1, "failures: {:?}", first.failures());

    // Step deadline (1 s) passes, then the 1 s retry backoff.
    clock.set(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(5));
    let second = claims
        .execute(claim_as("retry-owner", 60_000))
        .await
        .unwrap();

    assert!(second.failures().is_empty(), "{:?}", second.failures());
    assert_eq!(second.claims().len(), 1);
    assert_eq!(second.claims()[0].handler_request.attempt().get(), 2);
    let record = stream
        .load(&ProtocolId::new("timed-out").unwrap())
        .await
        .unwrap()
        .instance
        .step_record(&StepId::new("work").unwrap())
        .cloned()
        .unwrap();
    assert_eq!(record.status(), StepStatus::InProgress);
}
