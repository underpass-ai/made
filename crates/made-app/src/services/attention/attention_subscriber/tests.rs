//! The subscriber over a real recovery, resolver and projector, with
//! fakes of the ports underneath.
//!
//! In-crate doubles rather than the real adapters: `made-adapters`
//! depends on this crate, so borrowing its in-memory ledger would be a
//! cycle. What is under test is the wiring — that an append reaches
//! the ledger, that a ceremony nobody drives costs nothing, and that a
//! store which refuses cannot travel back up into the append.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_core::entities::ceremony_events::StepCompleted;
use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort,
    AgenticSystemExecutionUpdate, AgenticSystemPage, AgenticSystemQuery,
    AgenticSystemRepositoryPort, AgenticSystemSaveOutcome, AppendOutcome, BindOutcome,
    BindReplacement, CeremonyEventCursorPort, CeremonyEventStorePort, ClockPort,
    DeliveryFailureOutcome, EnqueueOutcome, HostActivationOutcome, HostActivationPort,
    HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPage, HostDeliveryPageLimit,
    HostDeliveryQuery, IntegratorBindingPort, LeasedDelivery, ProcessedOutcome, RecordedActivation,
    SupersessionOutcome,
};
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemRevision, AuditActor, AuditActorKind,
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventCursorLease,
    CeremonyEventCursorLeaseId, CeremonyEventPageLimit, CeremonyEventQuarantineReason, CeremonyId,
    CeremonyName, CeremonyVersion, DeliveryExpiryCause, DeliveryFailureReason, DurationMs, EventId,
    FollowReplacement, GlobalPosition, HostActivationAdapterKind, HostActivationEnvelope,
    HostAddress, HostAgentIncarnation, HostDeliveryId, HostDeliveryLease, HostDeliveryObservation,
    HostDeliveryRecord, HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorFence, IntegratorScope, ProcessedActionRef, RoleId, StepAttempt,
    StepId, StepIteration, StepOutput, StepResult, StreamVersion,
};
use time::OffsetDateTime;

use crate::services::attention::{
    AttentionAudienceResolver, AttentionProjector, AttentionRecovery,
};

use super::*;
use crate::services::attention as made_app_definition_lookup;

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn ceremony(id: &str) -> CeremonyId {
    CeremonyId::new(id).expect("a valid ceremony id")
}

fn binding(id: &str, ceremony_id: &str) -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new(id).expect("a valid binding id"),
        IntegratorScope::ceremony(ceremony(ceremony_id)),
        RoleId::new("INTEGRATOR").expect("a valid role"),
        HostDestination::new(
            HostKind::new("claude-code").expect("a valid host kind"),
            HostAddress::new("session-1").expect("a valid address"),
            made_core::value_objects::HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").expect("a valid incarnation"),
        now(),
    )
}

fn appended(position: u64, ceremony_id: &str) -> Vec<PositionedRecord> {
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(format!("e-{position}")).expect("a valid event id"),
        event: CeremonyEvent::StepCompleted(StepCompleted {
            step_id: StepId::new("implement").expect("a valid step id"),
            state_visit: None,
            state_iteration: None,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(StepOutput::default()).expect("a valid result"),
            next_iteration: None,
            finished_by: RoleId::new("IMPLEMENTER").expect("a valid role"),
            finished_at: now(),
        }),
        ceremony_id: ceremony(ceremony_id),
        definition_name: CeremonyName::new("review").expect("a valid name"),
        definition_version: CeremonyVersion::v1(),
        occurred_at: now(),
        actor: AuditActor::new("test", AuditActorKind::Engine, None).expect("a valid actor"),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .expect("a valid record");
    vec![PositionedRecord {
        position: GlobalPosition::new(position).expect("a valid position"),
        record,
    }]
}

fn subscriber(
    bindings: Vec<IntegratorBinding>,
    feed: Vec<PositionedRecord>,
    ledger: Arc<LedgerFake>,
) -> AttentionSubscriber {
    let projector = AttentionProjector::new(
        Arc::new(FeedFake { records: feed }),
        Arc::new(CursorFake::default()),
        ledger,
        Arc::new(ActivationFake),
        Arc::new(NoDefinitionsFake),
        Arc::new(FrozenClock),
    );
    let resolver = AttentionAudienceResolver::new(
        Arc::new(ExecutionStoreFake),
        Arc::new(SystemRepositoryFake),
    );
    AttentionSubscriber::new(Arc::new(AttentionRecovery::new(
        Arc::new(BindingsFake { bindings }),
        Arc::new(resolver),
        Arc::new(projector),
    )))
}

#[tokio::test]
async fn an_append_leaves_the_bound_integrator_something_to_collect() {
    let ledger = Arc::new(LedgerFake::default());
    let records = appended(1, "loop-1");
    let subscriber = subscriber(
        vec![binding("b-1", "loop-1")],
        records.clone(),
        Arc::clone(&ledger),
    );

    subscriber.observe(&records).await;

    let held = ledger.records();
    assert_eq!(held.len(), 1, "nobody had to enqueue this by hand");
    assert_eq!(
        held[0].target(),
        &HostDeliveryTarget::IntegratorBinding {
            binding_id: IntegratorBindingId::new("b-1").expect("a valid binding id")
        }
    );
}

#[tokio::test]
async fn a_ceremony_nobody_drives_leaves_the_ledger_alone() {
    let ledger = Arc::new(LedgerFake::default());
    let records = appended(1, "loop-1");
    let subscriber = subscriber(
        vec![binding("b-1", "somebody-elses")],
        records.clone(),
        Arc::clone(&ledger),
    );

    subscriber.observe(&records).await;

    assert!(
        ledger.records().is_empty(),
        "a binding on another scope is not this ceremony's audience"
    );
}

#[tokio::test]
async fn a_revoked_binding_is_no_longer_an_audience() {
    let ledger = Arc::new(LedgerFake::default());
    let records = appended(1, "loop-1");
    let revoked = binding("b-1", "loop-1").revoked(now());
    let subscriber = subscriber(vec![revoked], records.clone(), Arc::clone(&ledger));

    subscriber.observe(&records).await;

    assert!(
        ledger.records().is_empty(),
        "offering work to a host that was replaced is worse than offering none"
    );
}

#[tokio::test]
async fn a_ledger_that_refuses_does_not_come_back_up_into_the_append() {
    let ledger = Arc::new(LedgerFake::refusing());
    let records = appended(1, "loop-1");
    let subscriber = subscriber(
        vec![binding("b-1", "loop-1")],
        records.clone(),
        Arc::clone(&ledger),
    );

    // `observe` returns nothing at all: the proof is that this call
    // completes, having swallowed a store that answered with an error.
    subscriber.observe(&records).await;

    assert!(ledger.records().is_empty());
}

#[tokio::test]
async fn an_append_that_sealed_nothing_asks_no_store_anything() {
    let ledger = Arc::new(LedgerFake::default());
    let subscriber = subscriber(Vec::new(), Vec::new(), Arc::clone(&ledger));

    subscriber.observe(&[]).await;

    assert!(ledger.records().is_empty());
}

// ---------------------------------------------------------------- fakes

/// Nothing here exercises a reading that needs a definition.
#[derive(Debug)]
struct NoDefinitionsFake;

#[async_trait]
impl made_app_definition_lookup::CeremonyDefinitionLookup for NoDefinitionsFake {
    async fn definition_of(
        &self,
        _ceremony_id: &CeremonyId,
    ) -> Result<Option<made_core::entities::CeremonyDefinition>, DomainError> {
        Ok(None)
    }
}

struct FrozenClock;

impl ClockPort for FrozenClock {
    fn now(&self) -> OffsetDateTime {
        now()
    }
}

struct BindingsFake {
    bindings: Vec<IntegratorBinding>,
}

#[async_trait]
impl IntegratorBindingPort for BindingsFake {
    /// Nothing under test here writes a loop's mark.
    async fn record_progress(
        &self,
        _id: &IntegratorBindingId,
        _progress: made_core::value_objects::LoopProgressMark,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        Ok(None)
    }

    async fn bind(
        &self,
        _binding: IntegratorBinding,
        _replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        unimplemented!("the subscriber never binds")
    }

    async fn current(
        &self,
        _scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("the subscriber does not know which scope to ask for")
    }

    async fn revoke(
        &self,
        _id: &IntegratorBindingId,
        _now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("the subscriber never revokes")
    }

    async fn list(
        &self,
        _scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        Ok(self.bindings.clone())
    }
}

struct FeedFake {
    records: Vec<PositionedRecord>,
}

#[async_trait]
impl CeremonyEventStorePort for FeedFake {
    async fn append(
        &self,
        _stream: &CeremonyId,
        _expected: StreamVersion,
        _facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        unimplemented!("a projection never appends")
    }

    async fn read(
        &self,
        _stream: &CeremonyId,
        _from: StreamVersion,
        _limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        unimplemented!("the projector reads the global feed, not one stream")
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        Ok(self
            .records
            .iter()
            .filter(|record| record.position >= from)
            .take(limit.value())
            .cloned()
            .collect())
    }

    async fn head(&self, _stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        unimplemented!("the projector does not ask for a stream head")
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        unimplemented!("the projector does not enumerate streams")
    }
}

#[derive(Default)]
struct CursorFake {
    acknowledged: Mutex<Option<GlobalPosition>>,
}

#[async_trait]
impl CeremonyEventCursorPort for CursorFake {
    async fn position(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        Ok(*self.acknowledged.lock().expect("the cursor is readable"))
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError> {
        Ok(Some(CeremonyEventCursorLease::new(
            consumer.clone(),
            lease_id,
            *self.acknowledged.lock().expect("the cursor is readable"),
            CeremonyEventCursorAttempt::NONE,
            now + time::Duration::milliseconds(duration.get() as i64),
        )))
    }

    async fn acknowledge(
        &self,
        _consumer: &CeremonyEventConsumer,
        _through: GlobalPosition,
    ) -> Result<(), DomainError> {
        unimplemented!("the projector acknowledges under its lease")
    }

    async fn acknowledge_lease(
        &self,
        _lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(through);
        Ok(())
    }

    async fn mark_failed(
        &self,
        _lease: &CeremonyEventCursorLease,
        _position: GlobalPosition,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantine(
        &self,
        _lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        _reason: CeremonyEventQuarantineReason,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(position);
        Ok(())
    }

    async fn release(&self, _lease: &CeremonyEventCursorLease) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantined(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<made_core::value_objects::QuarantinedCeremonyEvent>, DomainError> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct LedgerFake {
    held: Mutex<BTreeMap<HostDeliveryId, HostDeliveryRecord>>,
    refuses: bool,
}

impl LedgerFake {
    fn refusing() -> Self {
        Self {
            held: Mutex::default(),
            refuses: true,
        }
    }

    fn records(&self) -> Vec<HostDeliveryRecord> {
        self.held
            .lock()
            .expect("the ledger is readable")
            .values()
            .cloned()
            .collect()
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        if self.refuses {
            return Err(DomainError::InvariantViolated {
                reason: "the delivery ledger is unreachable",
            });
        }
        let mut held = self.held.lock().expect("the ledger is writable");
        if let Some(existing) = held.get(record.id()) {
            return Ok(EnqueueOutcome::AlreadyQueued(existing.clone()));
        }
        held.insert(record.id().clone(), record.clone());
        Ok(EnqueueOutcome::Enqueued(record))
    }

    async fn lease(
        &self,
        _filter: &HostDeliveryFilter,
        _owner: &HostAgentIncarnation,
        _now: OffsetDateTime,
        _duration: DurationMs,
        _limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        unimplemented!("the projector offers work; it does not take it")
    }

    async fn acknowledge(
        &self,
        _lease: &HostDeliveryLease,
        _observation: &HostDeliveryObservation,
        _now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        unimplemented!("acknowledging is the host's half of the loop")
    }

    async fn mark_processed(
        &self,
        _delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        _action: &ProcessedActionRef,
        _now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        unimplemented!("processing is the host's half of the loop")
    }

    async fn record_activation(
        &self,
        _delivery_id: &HostDeliveryId,
        _outcome: &HostActivationOutcome,
        _now: OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        unimplemented!("a destination that is not woken records no activation")
    }

    async fn mark_failed(
        &self,
        _lease: &HostDeliveryLease,
        _reason: &DeliveryFailureReason,
        _now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        unimplemented!("the leased failure path is the host's")
    }

    async fn release(
        &self,
        _lease: &HostDeliveryLease,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        unimplemented!("the projector holds no lease")
    }

    async fn abandon(
        &self,
        _delivery_id: &HostDeliveryId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("nothing in these rounds fills a queue")
    }

    async fn expire(&self, _now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("the timeout sweep is not the projector's")
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("ending a ceremony's offers is not the projector's")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        _replacement: &HostDeliveryTarget,
        _follow: FollowReplacement,
        _now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        unimplemented!("replacing a destination is the binding's business")
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        Ok(self
            .held
            .lock()
            .expect("the ledger is readable")
            .get(id)
            .cloned())
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        let held = self.held.lock().expect("the ledger is readable");
        let records: Vec<HostDeliveryRecord> = held
            .values()
            .filter(|record| query.admits(record))
            .cloned()
            .collect();
        Ok(HostDeliveryPage::new(records, None))
    }
}

struct ActivationFake;

#[async_trait]
impl HostActivationPort for ActivationFake {
    async fn activate(
        &self,
        _binding: &IntegratorBinding,
        _record: &HostDeliveryRecord,
        _envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, DomainError> {
        unimplemented!("a destination bound with activation none is never woken")
    }

    fn kind(&self) -> HostActivationAdapterKind {
        HostActivationAdapterKind::None
    }
}

struct ExecutionStoreFake;

#[async_trait]
impl AgenticSystemExecutionStorePort for ExecutionStoreFake {
    async fn create(
        &self,
        _execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        unimplemented!("these bindings are scoped to a ceremony")
    }

    async fn update(
        &self,
        _execution: AgenticSystemExecution,
        _expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn list_by_system(
        &self,
        _id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        unimplemented!("the resolver asks for one run")
    }
}

struct SystemRepositoryFake;

#[async_trait]
impl AgenticSystemRepositoryPort for SystemRepositoryFake {
    async fn save(
        &self,
        _system: AgenticSystem,
        _expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemId,
        _revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        unimplemented!("these bindings are scoped to a ceremony")
    }

    async fn list(&self, _query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        unimplemented!("the resolver asks for one design")
    }
}
