use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryBudgetLedgerStore, InMemoryCeremonyDefinitionPublications,
    InMemoryCeremonyDefinitionRepository, InMemoryCeremonyEventStore,
};
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimInput, BudgetedStepClaimUseCase,
    StartBudgetedCeremonyInput, StartBudgetedCeremonyUseCase,
};
use made_app::services::SessionStream;
use made_app::usecases::{
    CompleteCeremonyStepInput, CompleteCeremonyStepUseCase, MountCeremonyDefinitionsUseCase,
    PauseCeremonyInput, PauseCeremonyUseCase, ResolveCeremonyDefinitionUseCase, StartCeremonyInput,
    StartCeremonyStepInput,
};
use made_core::entities::BudgetLedgerEvent;
use made_core::error::DomainError;
use made_core::ports::{
    BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetLedgerStorePort, BudgetReservationPage,
    ClockPort, NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActorKind, BudgetAccountId, BudgetLedgerVersion, BudgetLimits, BudgetMeasurement,
    BudgetPageLimit, BudgetQuantities, BudgetReservationEstimate, BudgetReservationId,
    BudgetTokenCount, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, CostMicros,
    CurrencyCode, DurationMs, ExecutionDuration, IdempotencyKey, LeaseOwnerId, LifecycleReason,
    RoleId, StepId, StepOutput, StepResult, StepStatus, ToolCallCount,
};
use time::OffsetDateTime;
use tokio::sync::Notify;

const DEFINITION: &str = r#"
version: "1.0"
name: "budget_race"
description: "Budget admission race fixture"
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
retry_policies:
  default:
    max_attempts: 1
    backoff_seconds: 0
"#;

#[derive(Debug)]
struct FixedClock;

impl ClockPort for FixedClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }
}

#[derive(Debug)]
struct PauseAfterReservationStore {
    inner: InMemoryBudgetLedgerStore,
    pause_next_reservation: AtomicBool,
    reserved: Notify,
    resume: Notify,
}

impl PauseAfterReservationStore {
    fn new(pause_next_reservation: bool) -> Self {
        Self {
            inner: InMemoryBudgetLedgerStore::new(),
            pause_next_reservation: AtomicBool::new(pause_next_reservation),
            reserved: Notify::new(),
            resume: Notify::new(),
        }
    }

    async fn wait_until_reserved(&self) {
        self.reserved.notified().await;
    }

    fn resume_claim(&self) {
        self.resume.notify_one();
    }
}

#[async_trait]
impl BudgetLedgerStorePort for PauseAfterReservationStore {
    async fn load(
        &self,
        account: &BudgetAccountId,
    ) -> Result<Option<BudgetLedgerSnapshot>, DomainError> {
        self.inner.load(account).await
    }

    async fn append(
        &self,
        account: &BudgetAccountId,
        expected: BudgetLedgerVersion,
        events: Vec<BudgetLedgerEvent>,
    ) -> Result<BudgetAppendOutcome, DomainError> {
        let reserved = events
            .iter()
            .any(|event| matches!(event, BudgetLedgerEvent::Reserved { .. }));
        let outcome = self.inner.append(account, expected, events).await?;
        if reserved && self.pause_next_reservation.swap(false, Ordering::SeqCst) {
            self.reserved.notify_one();
            self.resume.notified().await;
        }
        Ok(outcome)
    }

    async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, DomainError> {
        self.inner.pending(after, limit).await
    }
}

struct BudgetRaceFixture {
    ceremony_id: CeremonyId,
    stream: Arc<SessionStream>,
    resolver: Arc<ResolveCeremonyDefinitionUseCase>,
    clock: Arc<FixedClock>,
    budgets: BudgetLedgerService,
    store: Arc<PauseAfterReservationStore>,
}

impl BudgetRaceFixture {
    async fn new(pause_next_reservation: bool) -> Self {
        std::fs::create_dir_all("tmp").unwrap();
        let directory = tempfile::tempdir_in("tmp").unwrap();
        std::fs::write(directory.path().join("budget_race.yaml"), DEFINITION).unwrap();
        let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
        MountCeremonyDefinitionsUseCase::new(
            Arc::new(FileSystemCeremonyDefinitionSource::from_directory(directory.path()).unwrap()),
            definitions.clone(),
        )
        .execute()
        .await
        .unwrap();
        let name = CeremonyName::new("budget_race").unwrap();
        let version = CeremonyVersion::v1();
        let definition = made_core::ports::CeremonyDefinitionRepositoryPort::get(
            definitions.as_ref(),
            &name,
            &version,
        )
        .await
        .unwrap();
        let publications = Arc::new(InMemoryCeremonyDefinitionPublications::new());
        made_core::ports::CeremonyDefinitionPublicationPort::publish(
            publications.as_ref(),
            made_core::entities::PublishedCeremonyDefinition::seal(definition).unwrap(),
        )
        .await
        .unwrap();
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        let stream = Arc::new(SessionStream::new(
            events.clone(),
            events,
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let clock = Arc::new(FixedClock);
        let store = Arc::new(PauseAfterReservationStore::new(pause_next_reservation));
        let budgets = BudgetLedgerService::new(store.clone(), clock.clone());
        let ceremony_id = CeremonyId::new("budget-race").unwrap();
        StartBudgetedCeremonyUseCase::new(
            publications.clone(),
            stream.clone(),
            clock.clone(),
            Arc::new(ForgetfulMemory::new()),
            budgets.clone(),
        )
        .execute(StartBudgetedCeremonyInput::new(
            StartCeremonyInput::new(
                ceremony_id.clone(),
                name,
                version,
                CeremonyContext::empty(),
                "operator",
                AuditActorKind::Service,
            ),
            limits(),
        ))
        .await
        .unwrap();
        Self {
            ceremony_id,
            stream,
            resolver: Arc::new(ResolveCeremonyDefinitionUseCase::new(
                definitions,
                publications,
            )),
            clock,
            budgets,
            store,
        }
    }

    fn claim_usecase(&self) -> Arc<BudgetedStepClaimUseCase> {
        Arc::new(BudgetedStepClaimUseCase::new(
            self.resolver.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.budgets.clone(),
        ))
    }

    fn claim_input(&self) -> BudgetedStepClaimInput {
        BudgetedStepClaimInput::new(
            StartCeremonyStepInput::new(
                self.ceremony_id.clone(),
                RoleId::new("WORKER").unwrap(),
                AuditActorKind::Agent,
                StepId::new("work").unwrap(),
                LeaseOwnerId::new("race-worker").unwrap(),
                IdempotencyKey::new("race-claim").unwrap(),
                DurationMs::from_millis(60_000),
            ),
            estimate(),
        )
    }

    async fn pause(&self) {
        PauseCeremonyUseCase::new(
            self.resolver.clone(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(PauseCeremonyInput::new(
            self.ceremony_id.clone(),
            "operator",
            AuditActorKind::Human,
            LifecycleReason::new("race").unwrap(),
        ))
        .await
        .unwrap();
    }
}

fn limits() -> BudgetLimits {
    BudgetLimits::new(
        BudgetQuantities::new(
            ExecutionDuration::from_micros(10_000),
            BudgetTokenCount::new(100),
            CostMicros::new(100),
            ToolCallCount::new(10),
        ),
        Some(CurrencyCode::new("EUR").unwrap()),
    )
    .unwrap()
}

fn estimate() -> BudgetReservationEstimate {
    BudgetReservationEstimate::new(
        BudgetMeasurement::Estimated(ExecutionDuration::from_micros(1_000)),
        BudgetMeasurement::Estimated(BudgetTokenCount::new(10)),
        BudgetMeasurement::Estimated(CostMicros::new(5)),
        BudgetMeasurement::Estimated(ToolCallCount::new(1)),
    )
}

#[tokio::test]
async fn pause_winning_after_reservation_keeps_the_orphan_charged() {
    let fixture = Arc::new(BudgetRaceFixture::new(true).await);
    let claim = fixture.claim_usecase();
    let input = fixture.claim_input();
    let running = tokio::spawn(async move { claim.execute(input).await });

    fixture.store.wait_until_reserved().await;
    fixture.pause().await;
    fixture.store.resume_claim();
    assert!(running.await.unwrap().is_err());

    let instance = fixture
        .stream
        .load(&fixture.ceremony_id)
        .await
        .unwrap()
        .instance;
    assert!(instance.is_paused());
    assert_eq!(
        instance
            .step_record(&StepId::new("work").unwrap())
            .unwrap()
            .status(),
        StepStatus::Pending
    );
    assert_eq!(
        fixture
            .budgets
            .pending(None, BudgetPageLimit::new(10).unwrap())
            .await
            .unwrap()
            .reservations()
            .len(),
        1
    );
}

#[tokio::test]
async fn claim_winning_before_pause_can_drain_while_the_reservation_stays_charged() {
    let fixture = BudgetRaceFixture::new(false).await;
    let claim = fixture
        .claim_usecase()
        .execute(fixture.claim_input())
        .await
        .unwrap();
    fixture.pause().await;

    let completed = CompleteCeremonyStepUseCase::new(
        fixture.resolver.clone(),
        fixture.stream.clone(),
        fixture.clock.clone(),
    )
    .execute(CompleteCeremonyStepInput::new(
        fixture.ceremony_id.clone(),
        StepId::new("work").unwrap(),
        StepResult::completed(StepOutput::empty()).unwrap(),
        AuditActorKind::Agent,
        claim.claim().claim_fence().clone(),
    ))
    .await
    .unwrap();

    assert!(completed.is_paused());
    assert_eq!(
        completed
            .step_record(&StepId::new("work").unwrap())
            .unwrap()
            .status(),
        StepStatus::Completed
    );
    let account = completed.budget_account_id().unwrap();
    assert_eq!(
        fixture
            .budgets
            .report(account)
            .await
            .unwrap()
            .reserved()
            .tokens()
            .value(),
        10
    );
}
