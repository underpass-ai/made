use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::ceremony_commands::StartStep;
use made_core::entities::{
    AuditFact, AuditRecord, CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance,
    PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, CeremonyEventStorePort, ClockPort, NoopCeremonyEventSubscriber, PositionedRecord,
};
use made_core::value_objects::{
    AuditActorKind, AuditEventType, BudgetAccountId, BudgetOperationId, BudgetReservationId,
    CeremonyContext, CeremonyEventPageLimit, CeremonyId, ExecutionOperationId, GlobalPosition,
    IdempotencyKey, LeaseOwnerId, MaxParallel, StepLease, StreamVersion,
};
use time::OffsetDateTime;
use tokio::sync::Barrier;

use super::*;
use crate::usecases::ceremony_test_support::{
    a_memory, ceremony_id, child_spawning_definition, lease_ttl, now, resolver_with,
    review_child_definition, role_id, started_instance, step_id, EventStoreFake, FixedClock,
    PublicationsFake,
};
use crate::usecases::{StartCeremonyStepInput, StartCeremonyStepUseCase};

struct BarrierOnParentEvent {
    inner: Arc<EventStoreFake>,
    parent_id: CeremonyId,
    event_type: AuditEventType,
    barrier: Barrier,
}

#[async_trait]
impl CeremonyEventStorePort for BarrierOnParentEvent {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        if stream == &self.parent_id
            && facts
                .iter()
                .any(|fact| fact.event.event_type() == self.event_type)
        {
            self.barrier.wait().await;
        }
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
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

struct AdvancingClock {
    base: OffsetDateTime,
    seconds: AtomicI64,
}

impl AdvancingClock {
    fn new(base: OffsetDateTime) -> Self {
        Self {
            base,
            seconds: AtomicI64::new(0),
        }
    }
}

impl ClockPort for AdvancingClock {
    fn now(&self) -> OffsetDateTime {
        self.base + time::Duration::seconds(self.seconds.fetch_add(1, Ordering::SeqCst))
    }
}

fn budgeted_claimed_parent(
    definition: &CeremonyDefinition,
) -> (CeremonyInstance, made_core::value_objects::StepClaimFence) {
    let published = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
    let account_id = BudgetAccountId::for_root(&ceremony_id()).unwrap();
    let mut events = CeremonyInstance::decide_start_bound_budgeted(
        ceremony_id(),
        &published,
        CeremonyContext::empty(),
        account_id.clone(),
        None,
        now(),
    )
    .unwrap();
    let pending = CeremonyInstance::rehydrate(events.iter()).unwrap();
    let record = pending.step_record(&step_id()).unwrap();
    let operation_id = ExecutionOperationId::for_step(
        pending.id(),
        &step_id(),
        pending.current_state_visit(),
        pending.current_state_iteration(),
        record.iteration(),
    );
    let reservation_id = BudgetReservationId::for_operation(
        &account_id,
        &BudgetOperationId::for_execution(&operation_id),
    );
    let command = CeremonyCommand::StartStep(StartStep {
        role_id: Some(role_id()),
        step_id: step_id(),
        lease: StepLease::acquire(
            LeaseOwnerId::new("budgeted-parent").unwrap(),
            IdempotencyKey::new("budgeted-parent-claim").unwrap(),
            now(),
            lease_ttl(),
        )
        .unwrap(),
        now: now(),
        max_parallel_ceiling: MaxParallel::SERVER_MAX,
        budget_reservation_id: Some(reservation_id),
    });
    events.extend(pending.decide(&command, definition).unwrap());
    let parent = CeremonyInstance::rehydrate(events.iter()).unwrap();
    let fence = parent.step_claim_fence(&step_id()).unwrap();
    (parent, fence)
}

#[tokio::test]
async fn concurrent_prepare_retries_an_identical_durable_completion() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::ceremony_test_support::DefinitionRepositoryFake::new(definition.clone()),
    );
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(review_child_definition()).await;
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition)).await.unwrap();
    let resolver = resolver_with(definitions, publications.clone());
    let clock = Arc::new(FixedClock::new(now()));
    let plain_stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let claim = StartCeremonyStepUseCase::new(resolver.clone(), plain_stream, clock.clone())
        .execute(StartCeremonyStepInput::new(
            ceremony_id(),
            role_id(),
            AuditActorKind::Agent,
            step_id(),
            LeaseOwnerId::new("children-race").unwrap(),
            IdempotencyKey::new("children-race").unwrap(),
            lease_ttl(),
        ))
        .await
        .unwrap();
    let racing_stream = Arc::new(SessionStream::new(
        Arc::new(BarrierOnParentEvent {
            inner: store.clone(),
            parent_id: ceremony_id(),
            event_type: AuditEventType::StepCompleted,
            barrier: Barrier::new(2),
        }),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let prepare = PrepareCeremonyChildrenUseCase::new(
        resolver,
        publications,
        racing_stream,
        clock,
        a_memory(),
    );
    let input = PrepareCeremonyChildrenInput::new(
        ceremony_id(),
        step_id(),
        claim.claim_fence().clone(),
        AuditActorKind::Agent,
    );

    let (first, second) = tokio::join!(prepare.execute(input.clone()), prepare.execute(input));
    let first = first.unwrap();
    let second = second.unwrap();

    assert_eq!(first.group_id(), second.group_id());
    assert_eq!(first.child_ids(), second.child_ids());
    assert_eq!(first.result(), second.result());
    let parent_records = store.records(&ceremony_id()).await;
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        1
    );
    assert_eq!(store.records(&first.child_ids()[0]).await.len(), 1);
}

#[tokio::test]
async fn concurrent_prepare_adopts_the_winning_plan_when_candidates_differ() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::ceremony_test_support::DefinitionRepositoryFake::new(definition.clone()),
    );
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(review_child_definition()).await;
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition)).await.unwrap();
    let resolver = resolver_with(definitions, publications.clone());
    let plain_stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let claim = StartCeremonyStepUseCase::new(
        resolver.clone(),
        plain_stream,
        Arc::new(FixedClock::new(now())),
    )
    .execute(StartCeremonyStepInput::new(
        ceremony_id(),
        role_id(),
        AuditActorKind::Agent,
        step_id(),
        LeaseOwnerId::new("children-plan-race").unwrap(),
        IdempotencyKey::new("children-plan-race").unwrap(),
        lease_ttl(),
    ))
    .await
    .unwrap();
    let racing_stream = Arc::new(SessionStream::new(
        Arc::new(BarrierOnParentEvent {
            inner: store.clone(),
            parent_id: ceremony_id(),
            event_type: AuditEventType::ChildSpawnPlanned,
            barrier: Barrier::new(2),
        }),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let prepare = PrepareCeremonyChildrenUseCase::new(
        resolver,
        publications,
        racing_stream,
        Arc::new(AdvancingClock::new(now())),
        a_memory(),
    );
    let input = PrepareCeremonyChildrenInput::new(
        ceremony_id(),
        step_id(),
        claim.claim_fence().clone(),
        AuditActorKind::Agent,
    );

    let (first, second) = tokio::join!(prepare.execute(input.clone()), prepare.execute(input));
    let first = first.unwrap();
    let second = second.unwrap();

    assert_eq!(first.group_id(), second.group_id());
    assert_eq!(first.child_ids(), second.child_ids());
    assert_eq!(first.result(), second.result());
    let parent_records = store.records(&ceremony_id()).await;
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        1
    );
    assert_eq!(store.records(&first.child_ids()[0]).await.len(), 1);
}

#[tokio::test]
async fn a_spawned_child_inherits_the_exact_root_budget_account() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::ceremony_test_support::DefinitionRepositoryFake::new(definition.clone()),
    );
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(definition.clone()).await;
    publications.seed(review_child_definition()).await;
    let (parent, claim_fence) = budgeted_claimed_parent(&definition);
    let account_id = parent.budget_account_id().cloned().unwrap();
    let store = Arc::new(EventStoreFake::default());
    store.save(&parent).await.unwrap();
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let prepare = PrepareCeremonyChildrenUseCase::new(
        resolver_with(definitions, publications.clone()),
        publications,
        stream,
        Arc::new(FixedClock::new(now())),
        a_memory(),
    );

    let output = prepare
        .execute(PrepareCeremonyChildrenInput::new(
            ceremony_id(),
            step_id(),
            claim_fence,
            AuditActorKind::Agent,
        ))
        .await
        .unwrap();
    let child = store.saved(&output.child_ids()[0]).await;

    assert_eq!(child.budget_account_id(), Some(&account_id));
    assert_eq!(child.lineage().unwrap().root_id(), parent.id());
}
