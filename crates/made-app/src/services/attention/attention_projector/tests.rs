//! The projector against fakes of the four ports it drives.
//!
//! In-crate doubles rather than the real adapters: `made-adapters`
//! depends on this crate, so borrowing its in-memory ledger would be a
//! cycle. The doubles implement what the projector uses and refuse the
//! rest loudly, so a projector that started calling something new is a
//! failing test rather than a quiet pass.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use made_core::entities::ceremony_events::StepCompleted;
use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
use made_core::ports::{
    AckOutcome, AppendOutcome, CeremonyEventCursorPort, CeremonyEventStorePort,
    DeliveryFailureOutcome, EnqueueOutcome, HostActivationOutcome, HostActivationPort,
    HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPage, HostDeliveryPageLimit,
    HostDeliveryQuery, LeasedDelivery, PositionedRecord, ProcessedOutcome, RecordedActivation,
    SupersessionOutcome,
};
use made_core::value_objects::{
    AttentionKind, AttentionPolicy, AuditActor, AuditActorKind, CeremonyEventConsumer,
    CeremonyEventCursorAttempt, CeremonyEventCursorLease, CeremonyEventCursorLeaseId,
    CeremonyEventPageLimit, CeremonyEventQuarantineReason, CeremonyId, CeremonyName,
    CeremonyVersion, DeliveryExpiryCause, DeliveryFailureReason, DurationMs, EventId,
    FollowReplacement, GlobalPosition, HostActivationAdapterKind, HostActivationEnvelope,
    HostActivationMode, HostActivationReceipt, HostAddress, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryItem, HostDeliveryLease, HostDeliveryObservation, HostDeliveryRecord,
    HostDeliveryStateKind, HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorFence, IntegratorScope, LoopLimits, ProcessedActionRef,
    QueueLimit, RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult, StreamVersion,
};
use time::OffsetDateTime;

use super::*;

const INTEGRATOR: &str = "INTEGRATOR";

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn ceremony(id: &str) -> CeremonyId {
    CeremonyId::new(id).unwrap()
}

fn positioned(position: u64, ceremony_id: &str, event: CeremonyEvent) -> PositionedRecord {
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(format!("e-{position}")).unwrap(),
        event,
        ceremony_id: ceremony(ceremony_id),
        definition_name: CeremonyName::new("review").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: now(),
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .unwrap();
    PositionedRecord {
        position: GlobalPosition::new(position).unwrap(),
        record,
    }
}

fn completed(step: &str) -> CeremonyEvent {
    CeremonyEvent::StepCompleted(StepCompleted {
        step_id: StepId::new(step).unwrap(),
        state_visit: None,
        state_iteration: None,
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(StepOutput::default()).unwrap(),
        next_iteration: None,
        finished_by: RoleId::new("IMPLEMENTER").unwrap(),
        finished_at: now(),
    })
}

fn binding(activation: HostActivationMode) -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        IntegratorScope::ceremony(ceremony("loop-1")),
        RoleId::new(INTEGRATOR).unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-1").unwrap(),
            activation,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        now(),
    )
}

fn audience(activation: HostActivationMode, policy: AttentionPolicy) -> AttentionAudience {
    AttentionAudience::new(
        binding(activation),
        policy,
        [ceremony("loop-1")].into_iter().collect(),
        None,
    )
}

fn policy_of(kinds: impl IntoIterator<Item = AttentionKind>, max_queued: u32) -> AttentionPolicy {
    AttentionPolicy::new(
        kinds,
        DurationMs::from_millis(5_000),
        QueueLimit::new(max_queued).unwrap(),
        made_core::value_objects::AttentionOverflowPolicy::DropOldestNonBlocking,
        None,
        LoopLimits::default(),
    )
    .unwrap()
}

fn projector(
    feed: Vec<PositionedRecord>,
    ledger: Arc<LedgerFake>,
    activation: Arc<ActivationFake>,
) -> AttentionProjector {
    AttentionProjector::new(
        Arc::new(FeedFake { records: feed }),
        Arc::new(CursorFake::default()),
        ledger,
        activation,
        Arc::new(FrozenClock),
    )
}

fn limit(value: usize) -> CeremonyEventPageLimit {
    CeremonyEventPageLimit::new(value).unwrap()
}

#[tokio::test]
async fn a_sealed_result_becomes_one_offer_to_the_bound_destination() {
    let ledger = Arc::new(LedgerFake::default());
    let projector = projector(
        vec![positioned(1, "loop-1", completed("implement"))],
        Arc::clone(&ledger),
        Arc::new(ActivationFake::unsupported()),
    );

    let round = projector
        .project(
            &audience(HostActivationMode::None, policy_of(AttentionKind::ALL, 200)),
            limit(8),
        )
        .await
        .unwrap();

    assert_eq!(round.queued, 1);
    assert_eq!(round.read, 1);
    let held = ledger.records();
    assert_eq!(held.len(), 1);
    assert_eq!(
        held[0].target(),
        &HostDeliveryTarget::IntegratorBinding {
            binding_id: IntegratorBindingId::new("b-1").unwrap()
        }
    );
}

#[tokio::test]
async fn a_ceremony_this_integrator_does_not_drive_moves_the_cursor_and_wakes_nobody() {
    let ledger = Arc::new(LedgerFake::default());
    let projector = projector(
        vec![
            positioned(1, "somebody-elses", completed("implement")),
            positioned(2, "loop-1", completed("implement")),
        ],
        Arc::clone(&ledger),
        Arc::new(ActivationFake::unsupported()),
    );

    let round = projector
        .project(
            &audience(HostActivationMode::None, policy_of(AttentionKind::ALL, 200)),
            limit(8),
        )
        .await
        .unwrap();

    assert_eq!(
        round.read, 2,
        "a cursor that stopped at another ceremony's record would never reach its own"
    );
    assert_eq!(round.queued, 1);
}

#[tokio::test]
async fn replaying_a_position_does_not_offer_the_same_news_twice() {
    let ledger = Arc::new(LedgerFake::default());
    let feed = vec![positioned(1, "loop-1", completed("implement"))];
    let cursors = Arc::new(CursorFake::default());
    let projector = AttentionProjector::new(
        Arc::new(FeedFake {
            records: feed.clone(),
        }),
        Arc::clone(&cursors) as Arc<dyn CeremonyEventCursorPort>,
        Arc::clone(&ledger) as Arc<dyn HostDeliveryLedgerPort>,
        Arc::new(ActivationFake::unsupported()),
        Arc::new(FrozenClock),
    );
    let audience = audience(HostActivationMode::None, policy_of(AttentionKind::ALL, 200));

    projector.project(&audience, limit(8)).await.unwrap();
    cursors.rewind();
    let again = projector.project(&audience, limit(8)).await.unwrap();

    assert_eq!(again.already_held, 1);
    assert_eq!(again.queued, 0);
    assert_eq!(ledger.records().len(), 1, "one piece of news, one delivery");
}

#[tokio::test]
async fn a_kind_the_policy_did_not_ask_for_is_not_offered() {
    let ledger = Arc::new(LedgerFake::default());
    let projector = projector(
        vec![positioned(1, "loop-1", completed("implement"))],
        Arc::clone(&ledger),
        Arc::new(ActivationFake::unsupported()),
    );

    let round = projector
        .project(
            &audience(
                HostActivationMode::None,
                policy_of([AttentionKind::HumanDecisionRequested], 200),
            ),
            limit(8),
        )
        .await
        .unwrap();

    assert_eq!(round.queued, 0);
    assert_eq!(round.read, 1, "the cursor still advanced past it");
}

#[tokio::test]
async fn a_destination_that_cannot_be_woken_is_not_woken() {
    let ledger = Arc::new(LedgerFake::default());
    let activation = Arc::new(ActivationFake::unsupported());
    let projector = projector(
        vec![positioned(1, "loop-1", completed("implement"))],
        Arc::clone(&ledger),
        Arc::clone(&activation),
    );

    projector
        .project(
            &audience(HostActivationMode::None, policy_of(AttentionKind::ALL, 200)),
            limit(8),
        )
        .await
        .unwrap();

    assert_eq!(
        activation.calls(),
        0,
        "a host that asks for its own work was rung up anyway"
    );
    assert_eq!(
        ledger.records()[0].state().kind(),
        HostDeliveryStateKind::Queued
    );
}

#[tokio::test]
async fn a_host_that_was_reached_is_written_down_as_reached() {
    let ledger = Arc::new(LedgerFake::default());
    let activation = Arc::new(ActivationFake::accepted());
    let projector = projector(
        vec![positioned(1, "loop-1", completed("implement"))],
        Arc::clone(&ledger),
        Arc::clone(&activation),
    );

    let round = projector
        .project(
            &audience(
                HostActivationMode::Command,
                policy_of(AttentionKind::ALL, 200),
            ),
            limit(8),
        )
        .await
        .unwrap();

    assert_eq!(round.activated, 1);
    assert_eq!(activation.calls(), 1);
    assert_eq!(
        ledger.records()[0].state().kind(),
        HostDeliveryStateKind::DeliveredToHost
    );
}

#[tokio::test]
async fn a_full_queue_sheds_the_oldest_result_and_says_that_it_did() {
    let ledger = Arc::new(LedgerFake::default());
    let projector = projector(
        vec![
            positioned(1, "loop-1", completed("first")),
            positioned(2, "loop-1", completed("second")),
        ],
        Arc::clone(&ledger),
        Arc::new(ActivationFake::unsupported()),
    );
    let audience = audience(HostActivationMode::None, policy_of(AttentionKind::ALL, 1));

    let round = projector.project(&audience, limit(8)).await.unwrap();

    assert_eq!(
        round.shed, 1,
        "a queue of one never made room for the second"
    );
    let kinds: Vec<AttentionKind> = ledger
        .records()
        .iter()
        .filter(|record| record.is_offerable_at(now()))
        .filter_map(|record| kind_of(record.item()))
        .collect();
    assert!(
        kinds.contains(&AttentionKind::Blocked),
        "the host was never told it had fallen behind: {kinds:?}"
    );
    assert!(
        ledger.records().iter().any(|record| matches!(
            record.state(),
            made_core::value_objects::HostDeliveryState::Expired {
                cause: DeliveryExpiryCause::QueueOverflow,
                ..
            }
        )),
        "what was dropped did not say why"
    );
}

fn kind_of(item: &HostDeliveryItem) -> Option<AttentionKind> {
    match item {
        HostDeliveryItem::Attention { attention_id, .. } => {
            AttentionKind::from_label(attention_id.as_str().rsplit(':').next()?)
        }
        HostDeliveryItem::Intervention { .. } => None,
    }
}

// ---------------------------------------------------------------- fakes

struct FrozenClock;

impl made_core::ports::ClockPort for FrozenClock {
    fn now(&self) -> OffsetDateTime {
        now()
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
    ) -> Result<AppendOutcome, made_core::error::DomainError> {
        unimplemented!("the projector never appends")
    }

    async fn read(
        &self,
        _stream: &CeremonyId,
        _from: StreamVersion,
        _limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, made_core::error::DomainError> {
        unimplemented!("the projector reads the global feed, not one stream")
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, made_core::error::DomainError> {
        Ok(self
            .records
            .iter()
            .filter(|record| record.position >= from)
            .take(limit.value())
            .cloned()
            .collect())
    }

    async fn head(
        &self,
        _stream: &CeremonyId,
    ) -> Result<StreamVersion, made_core::error::DomainError> {
        unimplemented!("the projector does not ask for a stream head")
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, made_core::error::DomainError> {
        unimplemented!("the projector does not enumerate streams")
    }
}

#[derive(Default)]
struct CursorFake {
    acknowledged: Mutex<Option<GlobalPosition>>,
}

impl CursorFake {
    /// Put the consumer back at the beginning, which is what a replay
    /// of the feed looks like from the projector's side.
    fn rewind(&self) {
        *self.acknowledged.lock().unwrap() = None;
    }
}

#[async_trait]
impl CeremonyEventCursorPort for CursorFake {
    async fn position(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, made_core::error::DomainError> {
        Ok(*self.acknowledged.lock().unwrap())
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, made_core::error::DomainError> {
        Ok(Some(CeremonyEventCursorLease::new(
            consumer.clone(),
            lease_id,
            *self.acknowledged.lock().unwrap(),
            CeremonyEventCursorAttempt::NONE,
            now + time::Duration::milliseconds(duration.get() as i64),
        )))
    }

    async fn acknowledge(
        &self,
        _consumer: &CeremonyEventConsumer,
        _through: GlobalPosition,
    ) -> Result<(), made_core::error::DomainError> {
        unimplemented!("the projector acknowledges under its lease")
    }

    async fn acknowledge_lease(
        &self,
        _lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), made_core::error::DomainError> {
        *self.acknowledged.lock().unwrap() = Some(through);
        Ok(())
    }

    async fn mark_failed(
        &self,
        _lease: &CeremonyEventCursorLease,
        _position: GlobalPosition,
    ) -> Result<(), made_core::error::DomainError> {
        Ok(())
    }

    async fn quarantine(
        &self,
        _lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        _reason: CeremonyEventQuarantineReason,
        _now: OffsetDateTime,
    ) -> Result<(), made_core::error::DomainError> {
        *self.acknowledged.lock().unwrap() = Some(position);
        Ok(())
    }

    async fn release(
        &self,
        _lease: &CeremonyEventCursorLease,
    ) -> Result<(), made_core::error::DomainError> {
        Ok(())
    }

    async fn quarantined(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<
        Vec<made_core::value_objects::QuarantinedCeremonyEvent>,
        made_core::error::DomainError,
    > {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct LedgerFake {
    held: Mutex<BTreeMap<HostDeliveryId, HostDeliveryRecord>>,
}

impl LedgerFake {
    fn records(&self) -> Vec<HostDeliveryRecord> {
        self.held.lock().unwrap().values().cloned().collect()
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(
        &self,
        record: HostDeliveryRecord,
    ) -> Result<EnqueueOutcome, made_core::error::DomainError> {
        let mut held = self.held.lock().unwrap();
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
    ) -> Result<Vec<LeasedDelivery>, made_core::error::DomainError> {
        unimplemented!("the projector offers work; it does not take it")
    }

    async fn acknowledge(
        &self,
        _lease: &HostDeliveryLease,
        _observation: &HostDeliveryObservation,
        _now: OffsetDateTime,
    ) -> Result<AckOutcome, made_core::error::DomainError> {
        unimplemented!("acknowledging is the host's half of the loop")
    }

    async fn mark_processed(
        &self,
        _delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        _action: &ProcessedActionRef,
        _now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, made_core::error::DomainError> {
        unimplemented!("processing is the host's half of the loop")
    }

    async fn record_activation(
        &self,
        delivery_id: &HostDeliveryId,
        outcome: &HostActivationOutcome,
        now: OffsetDateTime,
    ) -> Result<RecordedActivation, made_core::error::DomainError> {
        let mut held = self.held.lock().unwrap();
        let Some(record) = held.get(delivery_id) else {
            return Ok(RecordedActivation::Unknown);
        };
        match outcome {
            HostActivationOutcome::Unsupported => {
                Ok(RecordedActivation::NotAttempted(record.clone()))
            }
            HostActivationOutcome::Accepted(receipt) => {
                let next = record.delivered(receipt.clone(), now);
                held.insert(delivery_id.clone(), next.clone());
                Ok(RecordedActivation::Delivered(next))
            }
            HostActivationOutcome::Failed(reason) => {
                let next = record.failed(reason.clone(), now);
                held.insert(delivery_id.clone(), next.clone());
                Ok(RecordedActivation::Exhausted(next))
            }
        }
    }

    async fn mark_failed(
        &self,
        _lease: &HostDeliveryLease,
        _reason: &DeliveryFailureReason,
        _now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, made_core::error::DomainError> {
        unimplemented!("the leased failure path is the host's")
    }

    async fn release(
        &self,
        _lease: &HostDeliveryLease,
        _now: OffsetDateTime,
    ) -> Result<(), made_core::error::DomainError> {
        unimplemented!("the projector holds no lease")
    }

    async fn abandon(
        &self,
        delivery_id: &HostDeliveryId,
        cause: DeliveryExpiryCause,
        now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, made_core::error::DomainError> {
        let mut held = self.held.lock().unwrap();
        let Some(record) = held.get(delivery_id) else {
            return Ok(None);
        };
        if record.state().is_terminal() {
            return Ok(None);
        }
        let next = record.expired(cause, now);
        held.insert(delivery_id.clone(), next.clone());
        Ok(Some(next))
    }

    async fn expire(
        &self,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, made_core::error::DomainError> {
        unimplemented!("the timeout sweep is not the projector's")
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, made_core::error::DomainError> {
        unimplemented!("ending a ceremony's offers is not the projector's")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        _replacement: &HostDeliveryTarget,
        _follow: FollowReplacement,
        _now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, made_core::error::DomainError> {
        unimplemented!("replacing a destination is the binding's business")
    }

    async fn get(
        &self,
        id: &HostDeliveryId,
    ) -> Result<Option<HostDeliveryRecord>, made_core::error::DomainError> {
        Ok(self.held.lock().unwrap().get(id).cloned())
    }

    async fn list(
        &self,
        query: &HostDeliveryQuery,
    ) -> Result<HostDeliveryPage, made_core::error::DomainError> {
        let held = self.held.lock().unwrap();
        let records: Vec<HostDeliveryRecord> = held
            .values()
            .filter(|record| query.admits(record))
            .cloned()
            .collect();
        Ok(HostDeliveryPage::new(records, None))
    }
}

struct ActivationFake {
    answer: HostActivationOutcome,
    calls: Mutex<u32>,
}

impl ActivationFake {
    fn unsupported() -> Self {
        Self {
            answer: HostActivationOutcome::Unsupported,
            calls: Mutex::new(0),
        }
    }

    fn accepted() -> Self {
        Self {
            answer: HostActivationOutcome::Accepted(HostActivationReceipt::new(
                HostActivationAdapterKind::Command,
                now(),
                None,
            )),
            calls: Mutex::new(0),
        }
    }

    fn calls(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl HostActivationPort for ActivationFake {
    async fn activate(
        &self,
        _binding: &IntegratorBinding,
        _record: &HostDeliveryRecord,
        _envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, made_core::error::DomainError> {
        *self.calls.lock().unwrap() += 1;
        Ok(self.answer.clone())
    }

    fn kind(&self) -> HostActivationAdapterKind {
        HostActivationAdapterKind::Command
    }
}
