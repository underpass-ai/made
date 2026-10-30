use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::protocol_commands::StartStep;
use made_core::entities::{
    AuditFact, AuditRecord, ProtocolCommand, ProtocolDefinition, ProtocolEvent, ProtocolInstance,
    PublishedProtocolDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, ClockPort, NoopProtocolEventSubscriber, PositionedRecord, ProtocolEventStorePort,
};
use made_core::value_objects::{
    AuditActorKind, AuditEventType, BudgetAccountId, BudgetOperationId, BudgetReservationId,
    ExecutionOperationId, GlobalPosition, IdempotencyKey, LeaseOwnerId, MaxParallel,
    ProtocolContext, ProtocolEventPageLimit, ProtocolId, StepLease, StreamVersion,
};
use time::OffsetDateTime;
use tokio::sync::Barrier;

use super::*;
use crate::usecases::protocol_test_support::{
    a_memory, child_spawning_definition, lease_ttl, now, protocol_id, resolver_with,
    review_child_definition, role_id, started_instance, step_id, EventStoreFake, FixedClock,
    PublicationsFake,
};
use crate::usecases::{StartProtocolStepInput, StartProtocolStepUseCase};

struct BarrierOnParentEvent {
    inner: Arc<EventStoreFake>,
    parent_id: ProtocolId,
    event_type: AuditEventType,
    barrier: Barrier,
}

#[async_trait]
impl ProtocolEventStorePort for BarrierOnParentEvent {
    async fn append(
        &self,
        stream: &ProtocolId,
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
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &ProtocolId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<ProtocolId>, DomainError> {
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
    definition: &ProtocolDefinition,
) -> (ProtocolInstance, made_core::value_objects::StepClaimFence) {
    let published = PublishedProtocolDefinition::seal(definition.clone()).unwrap();
    let account_id = BudgetAccountId::for_root(&protocol_id()).unwrap();
    let mut events = ProtocolInstance::decide_start_bound_budgeted(
        protocol_id(),
        &published,
        ProtocolContext::empty(),
        account_id.clone(),
        None,
        now(),
    )
    .unwrap();
    let pending = ProtocolInstance::rehydrate(events.iter()).unwrap();
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
    let command = ProtocolCommand::StartStep(StartStep {
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
    let parent = ProtocolInstance::rehydrate(events.iter()).unwrap();
    let fence = parent.step_claim_fence(&step_id()).unwrap();
    (parent, fence)
}

#[tokio::test]
async fn concurrent_prepare_retries_an_identical_durable_completion() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(definition.clone()),
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
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let claim = StartProtocolStepUseCase::new(resolver.clone(), plain_stream, clock.clone())
        .execute(StartProtocolStepInput::new(
            protocol_id(),
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
            parent_id: protocol_id(),
            event_type: AuditEventType::StepCompleted,
            barrier: Barrier::new(2),
        }),
        store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let prepare = PrepareProtocolChildrenUseCase::new(
        resolver,
        publications,
        racing_stream,
        clock,
        a_memory(),
    );
    let input = PrepareProtocolChildrenInput::new(
        protocol_id(),
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
    let parent_records = store.records(&protocol_id()).await;
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(ProtocolEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(ProtocolEvent::StepCompleted(_))))
            .count(),
        1
    );
    assert_eq!(store.records(&first.child_ids()[0]).await.len(), 1);
}

#[tokio::test]
async fn concurrent_prepare_adopts_the_winning_plan_when_candidates_differ() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(definition.clone()),
    );
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(review_child_definition()).await;
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition)).await.unwrap();
    let resolver = resolver_with(definitions, publications.clone());
    let plain_stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let claim = StartProtocolStepUseCase::new(
        resolver.clone(),
        plain_stream,
        Arc::new(FixedClock::new(now())),
    )
    .execute(StartProtocolStepInput::new(
        protocol_id(),
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
            parent_id: protocol_id(),
            event_type: AuditEventType::ChildSpawnPlanned,
            barrier: Barrier::new(2),
        }),
        store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let prepare = PrepareProtocolChildrenUseCase::new(
        resolver,
        publications,
        racing_stream,
        Arc::new(AdvancingClock::new(now())),
        a_memory(),
    );
    let input = PrepareProtocolChildrenInput::new(
        protocol_id(),
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
    let parent_records = store.records(&protocol_id()).await;
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(ProtocolEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );
    assert_eq!(
        parent_records
            .iter()
            .filter(|record| matches!(record.event(), Some(ProtocolEvent::StepCompleted(_))))
            .count(),
        1
    );
    assert_eq!(store.records(&first.child_ids()[0]).await.len(), 1);
}

#[tokio::test]
async fn a_spawned_child_inherits_the_exact_root_budget_account() {
    let definition = child_spawning_definition();
    let definitions = Arc::new(
        crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(definition.clone()),
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
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let prepare = PrepareProtocolChildrenUseCase::new(
        resolver_with(definitions, publications.clone()),
        publications,
        stream,
        Arc::new(FixedClock::new(now())),
        a_memory(),
    );

    let output = prepare
        .execute(PrepareProtocolChildrenInput::new(
            protocol_id(),
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
