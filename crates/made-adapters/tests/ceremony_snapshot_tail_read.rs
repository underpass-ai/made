use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_adapters::memory::InMemoryCeremonyEventStore;
use made_app::services::SessionStream;
use made_core::entities::ceremony_events::{CeremonyInstanceStarted, StepCompleted, StepStarted};
use made_core::entities::{AuditFact, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, CeremonyEventStorePort, NoopCeremonyEventSubscriber, PositionedRecord,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyContext, CeremonyEventPageLimit, CeremonyId, CeremonyName,
    CeremonyVersion, DurationMs, EventId, GlobalPosition, IdempotencyKey, LeaseOwnerId, RoleId,
    StateId, StepAttempt, StepId, StepIteration, StepLease, StepOutput, StepResult, StepStatus,
    StreamVersion,
};
use time::{Duration, OffsetDateTime};

#[derive(Debug)]
struct TailOnlyEventStore {
    inner: Arc<InMemoryCeremonyEventStore>,
    earliest_after: StreamVersion,
    reads: Mutex<Vec<StreamVersion>>,
}

impl TailOnlyEventStore {
    fn new(inner: Arc<InMemoryCeremonyEventStore>, earliest_after: StreamVersion) -> Self {
        Self {
            inner,
            earliest_after,
            reads: Mutex::new(Vec::new()),
        }
    }

    fn reads(&self) -> Vec<StreamVersion> {
        self.reads.lock().expect("read observations").clone()
    }
}

#[async_trait]
impl CeremonyEventStorePort for TailOnlyEventStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
        self.reads.lock().expect("read observations").push(after);
        if after < self.earliest_after {
            return Err(DomainError::InvariantViolated {
                reason: "a snapshot-backed load read the journal prefix",
            });
        }
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.inner.streams().await
    }
}

#[tokio::test]
async fn load_reads_only_the_snapshot_head_and_event_tail() {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let writer = SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    );
    let ceremony_id = CeremonyId::new("snapshot-tail").unwrap();
    let definition_name = CeremonyName::new("snapshot_tail").unwrap();
    let step_id = StepId::new("work").unwrap();
    let role_id = RoleId::new("WORKER").unwrap();
    let opened_at = OffsetDateTime::UNIX_EPOCH;
    let actor = AuditActor::new("test", AuditActorKind::Engine, None).unwrap();

    let opened = writer
        .open(
            vec![opening_event(
                &ceremony_id,
                &definition_name,
                &step_id,
                opened_at,
            )],
            actor.clone(),
            opened_at,
        )
        .await
        .unwrap();

    let started_at = opened_at + Duration::seconds(1);
    let started = writer
        .commit(
            opened,
            vec![fact(
                &ceremony_id,
                &definition_name,
                actor.clone(),
                "snapshot-tail-step-started",
                CeremonyEvent::StepStarted(StepStarted {
                    step_id: step_id.clone(),
                    state_iteration: None,
                    iteration: StepIteration::FIRST,
                    attempt: StepAttempt::FIRST,
                    lease: StepLease::acquire(
                        LeaseOwnerId::new("worker-1").unwrap(),
                        IdempotencyKey::new("snapshot-tail-work-1").unwrap(),
                        started_at,
                        DurationMs::from_millis(60_000),
                    )
                    .unwrap(),
                    started_by: role_id.clone(),
                    role_from: None,
                    sealed_role: None,
                    started_at,
                }),
                started_at,
            )],
        )
        .await
        .unwrap();
    let snapshot_version = started.version;
    assert_eq!(snapshot_version, StreamVersion::new(2));

    let completed_at = opened_at + Duration::seconds(2);
    let tail_event_id = EventId::new("snapshot-tail-step-completed").unwrap();
    let mut tail = fact(
        &ceremony_id,
        &definition_name,
        actor,
        tail_event_id.as_str(),
        CeremonyEvent::StepCompleted(StepCompleted {
            step_id: step_id.clone(),
            state_iteration: None,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(StepOutput::empty()).unwrap(),
            next_iteration: None,
            finished_by: role_id,
            finished_at: completed_at,
        }),
        completed_at,
    );
    tail.correlation_id =
        Some(EventId::new("snapshot-tail:ceremony_instance_started:session").unwrap());
    tail.causation_id = started.head_event_id().cloned();
    store
        .append(&ceremony_id, snapshot_version, vec![tail])
        .await
        .unwrap();

    let expected_after = StreamVersion::new(snapshot_version.value().saturating_sub(1));
    let observed = Arc::new(TailOnlyEventStore::new(store.clone(), expected_after));
    let reader = SessionStream::new(
        observed.clone(),
        store,
        Arc::new(NoopCeremonyEventSubscriber),
    );

    let loaded = reader.load(&ceremony_id).await.unwrap();

    assert_eq!(observed.reads(), vec![expected_after]);
    assert_eq!(loaded.version, StreamVersion::new(3));
    assert_eq!(loaded.head_event_id(), Some(&tail_event_id));
    assert_eq!(
        loaded.instance.step_record(&step_id).unwrap().status(),
        StepStatus::Completed
    );
}

fn opening_event(
    ceremony_id: &CeremonyId,
    definition_name: &CeremonyName,
    step_id: &StepId,
    created_at: OffsetDateTime,
) -> CeremonyEvent {
    CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
        ceremony_id: ceremony_id.clone(),
        definition_name: definition_name.clone(),
        definition_version: CeremonyVersion::v1(),
        initial_state: StateId::new("WORKING").unwrap(),
        step_ids: BTreeSet::from([step_id.clone()]),
        context: CeremonyContext::empty(),
        bound_definition: None,
        created_at,
    })
}

fn fact(
    ceremony_id: &CeremonyId,
    definition_name: &CeremonyName,
    actor: AuditActor,
    event_id: &str,
    event: CeremonyEvent,
    occurred_at: OffsetDateTime,
) -> AuditFact {
    AuditFact {
        event_id: EventId::new(event_id).unwrap(),
        event,
        ceremony_id: ceremony_id.clone(),
        definition_name: definition_name.clone(),
        definition_version: CeremonyVersion::v1(),
        occurred_at,
        actor,
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}
