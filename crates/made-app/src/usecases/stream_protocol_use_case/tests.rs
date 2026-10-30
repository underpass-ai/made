use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use made_core::entities::protocol_events::{
    ProtocolCancelled, ProtocolCompleted, ProtocolDeadlineExceeded, ProtocolInstanceStarted,
    StateDeadlineExceeded, StepDeadlineExceeded,
};
use made_core::entities::{AuditFact, AuditRecord, ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, PositionedRecord, ProtocolEventStorePort, ProtocolProgressNotifierPort,
    ProtocolProgressSubscriptionPort,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, GlobalPosition, LifecycleReason, ProtocolDeadline,
    ProtocolEventPageLimit, ProtocolId, ProtocolProgressWait, RoleId, StateDeadline, StateId,
    StateIteration, StateVisit, StepAttempt, StepClaimFence, StepDeadline, StepId, StepIteration,
    StepResult, StreamVersion,
};

use super::*;
use crate::usecases::protocol_test_support::{
    definition, now, protocol_id, started_instance, EventStoreFake,
};

#[derive(Debug)]
struct TrackingNotifier {
    active: Arc<AtomicUsize>,
    immediately_ready: bool,
}

impl TrackingNotifier {
    fn new(immediately_ready: bool) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            immediately_ready,
        }
    }
}

impl ProtocolProgressNotifierPort for TrackingNotifier {
    fn subscribe(&self) -> Box<dyn ProtocolProgressSubscriptionPort> {
        self.active.fetch_add(1, Ordering::SeqCst);
        Box::new(TrackingSubscription {
            active: self.active.clone(),
            immediately_ready: self.immediately_ready,
        })
    }
}

#[derive(Debug)]
struct TrackingSubscription {
    active: Arc<AtomicUsize>,
    immediately_ready: bool,
}

impl Drop for TrackingSubscription {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl ProtocolProgressSubscriptionPort for TrackingSubscription {
    async fn wait(&mut self) {
        if self.immediately_ready {
            tokio::task::yield_now().await;
        } else {
            futures::future::pending::<()>().await;
        }
    }
}

fn input(after: u64, max: usize, wait_ms: u32) -> StreamProtocolInput {
    StreamProtocolInput::new(
        protocol_id(),
        StreamVersion::new(after),
        ProtocolEventPageLimit::new(max).unwrap(),
        ProtocolProgressWait::from_millis(wait_ms).unwrap(),
    )
}

fn started_fact(instance: &ProtocolInstance, suffix: &str) -> AuditFact {
    fact(
        instance,
        suffix,
        ProtocolEvent::ProtocolInstanceStarted(ProtocolInstanceStarted {
            protocol_id: instance.id().clone(),
            definition_name: instance.definition_name().clone(),
            definition_version: instance.definition_version().clone(),
            initial_state: instance.current_state().clone(),
            step_ids: instance.step_records().keys().cloned().collect(),
            context: instance.context().clone(),
            bound_definition: instance.bound_definition(),
            lineage: None,
            succession: None,
            budget_account_id: None,
            protocol_deadline: instance.protocol_deadline(),
            state_deadline: instance.state_deadline().cloned(),
            created_at: instance.created_at(),
        }),
    )
}

fn completed_fact(instance: &ProtocolInstance, suffix: &str) -> AuditFact {
    fact(
        instance,
        suffix,
        ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
            final_state: StateId::new("COMPLETED").unwrap(),
            completed_at: now(),
        }),
    )
}

fn fact(instance: &ProtocolInstance, suffix: &str, event: ProtocolEvent) -> AuditFact {
    AuditFact {
        event_id: EventId::new(format!("progress-{suffix}")).unwrap(),
        event,
        protocol_id: instance.id().clone(),
        definition_name: instance.definition_name().clone(),
        definition_version: instance.definition_version().clone(),
        occurred_at: now(),
        actor: AuditActor::new("progress-test", AuditActorKind::Service, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

async fn one_record_store() -> (Arc<EventStoreFake>, ProtocolInstance) {
    let definition = definition();
    let instance = started_instance(&definition);
    let store = Arc::new(EventStoreFake::default());
    store.save(&instance).await.unwrap();
    (store, instance)
}

#[tokio::test]
async fn an_unknown_protocol_fails_before_a_stream_is_returned() {
    let store = Arc::new(EventStoreFake::default());
    let notifier = Arc::new(TrackingNotifier::new(false));

    let error = StreamProtocolUseCase::new(store, notifier)
        .execute(input(0, 1, 0))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        DomainError::NotFound {
            what: "protocol_instance"
        }
    );
}

#[tokio::test(start_paused = true)]
async fn continuous_wakes_cannot_starve_the_wait_deadline() {
    let (store, _) = one_record_store().await;
    let notifier = Arc::new(TrackingNotifier::new(true));
    let usecase = StreamProtocolUseCase::new(store, notifier);
    let mut stream = usecase.execute(input(1, 1, 10)).await.unwrap();

    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(10)).await;
    let frame = stream.next().await.unwrap().unwrap();

    let ProtocolProgressFrame::End(end) = frame else {
        panic!("deadline must finish the caught-up stream");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::WaitElapsed);
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(1));
}

#[tokio::test]
async fn dropping_a_stream_aborts_a_store_read_in_flight() {
    let (store, _) = one_record_store().await;
    let active_reads = Arc::new(AtomicUsize::new(0));
    let blocking = Arc::new(BlockingReadStore {
        inner: store,
        calls: AtomicUsize::new(0),
        active_reads: active_reads.clone(),
    });
    let notifier = Arc::new(TrackingNotifier::new(true));
    let stream = StreamProtocolUseCase::new(blocking, notifier)
        .execute(input(1, 1, 30_000))
        .await
        .unwrap();

    while active_reads.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    drop(stream);
    tokio::task::yield_now().await;

    assert_eq!(active_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dropping_a_stream_blocked_on_backpressure_releases_its_subscription() {
    let (store, instance) = one_record_store().await;
    store
        .append(
            &protocol_id(),
            StreamVersion::new(1),
            vec![
                started_fact(&instance, "second"),
                started_fact(&instance, "third"),
            ],
        )
        .await
        .unwrap();
    let notifier = Arc::new(TrackingNotifier::new(false));
    let settings = ProtocolProgressSettings::new(std::time::Duration::from_millis(250), 1).unwrap();
    let stream = StreamProtocolUseCase::with_settings(store, notifier.clone(), settings)
        .execute(input(0, 3, 30_000))
        .await
        .unwrap();

    tokio::task::yield_now().await;
    assert_eq!(notifier.active.load(Ordering::SeqCst), 1);
    drop(stream);
    tokio::task::yield_now().await;

    assert_eq!(notifier.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_terminal_append_between_empty_read_and_head_is_delivered_before_end() {
    let (store, instance) = one_record_store().await;
    let racing = Arc::new(TerminalReadRaceStore {
        inner: store,
        instance,
        raced: AtomicBool::new(false),
    });
    let notifier = Arc::new(TrackingNotifier::new(false));
    let mut stream = StreamProtocolUseCase::new(racing, notifier)
        .execute(input(1, 1, 0))
        .await
        .unwrap();

    let record = stream.next().await.unwrap().unwrap();
    let ProtocolProgressFrame::Record(record) = record else {
        panic!("the raced terminal record must be replayed");
    };
    assert_eq!(record.sequence().value(), 2);

    let end = stream.next().await.unwrap().unwrap();
    let ProtocolProgressFrame::End(end) = end else {
        panic!("terminal replay must be followed by End");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::Terminal);
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(2));
}

#[tokio::test]
async fn terminal_history_remains_terminal_when_late_records_follow_it() {
    let (store, instance) = one_record_store().await;
    store
        .append(
            &protocol_id(),
            StreamVersion::new(1),
            vec![
                completed_fact(&instance, "terminal-before-late"),
                started_fact(&instance, "late-fact"),
            ],
        )
        .await
        .unwrap();
    let notifier = Arc::new(TrackingNotifier::new(false));
    let usecase = StreamProtocolUseCase::new(store, notifier);

    let mut through_terminal = usecase.execute(input(0, 3, 0)).await.unwrap();
    assert!(matches!(
        through_terminal.next().await.unwrap().unwrap(),
        ProtocolProgressFrame::Record(record) if record.sequence().value() == 1
    ));
    assert!(matches!(
        through_terminal.next().await.unwrap().unwrap(),
        ProtocolProgressFrame::Record(record) if record.sequence().value() == 2
    ));
    let ProtocolProgressFrame::End(end) = through_terminal.next().await.unwrap().unwrap() else {
        panic!("terminal record must close this follow request");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::Terminal);
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(2));
    assert_eq!(end.head_sequence(), StreamVersion::new(3));

    let mut late_suffix = usecase.execute(input(2, 1, 0)).await.unwrap();
    assert!(matches!(
        late_suffix.next().await.unwrap().unwrap(),
        ProtocolProgressFrame::Record(record) if record.sequence().value() == 3
    ));
    let ProtocolProgressFrame::End(end) = late_suffix.next().await.unwrap().unwrap() else {
        panic!("late suffix remains part of a terminal journal");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::Terminal);
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(3));

    let mut caught_up = usecase.execute(input(3, 1, 0)).await.unwrap();
    let ProtocolProgressFrame::End(end) = caught_up.next().await.unwrap().unwrap() else {
        panic!("resuming at a late head must still report terminal");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::Terminal);
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(3));
}

#[tokio::test]
async fn every_lifecycle_terminal_remains_terminal_when_a_late_record_follows() {
    for (label, terminal) in lifecycle_terminal_events() {
        let (store, instance) = one_record_store().await;
        store
            .append(
                &protocol_id(),
                StreamVersion::new(1),
                vec![
                    fact(&instance, &format!("{label}-terminal"), terminal),
                    started_fact(&instance, &format!("{label}-late")),
                ],
            )
            .await
            .unwrap();
        let notifier = Arc::new(TrackingNotifier::new(false));
        let usecase = StreamProtocolUseCase::new(store, notifier);

        let mut suffix = usecase.execute(input(2, 1, 0)).await.unwrap();
        assert!(matches!(
            suffix.next().await.unwrap().unwrap(),
            ProtocolProgressFrame::Record(record) if record.sequence().value() == 3
        ));
        let ProtocolProgressFrame::End(end) = suffix.next().await.unwrap().unwrap() else {
            panic!("{label} late suffix must finish with an end frame");
        };
        assert_eq!(
            end.reason(),
            ProtocolProgressEndReason::Terminal,
            "{label} history lost terminality"
        );
        assert_eq!(end.resume_after_sequence(), StreamVersion::new(3));
    }
}

#[tokio::test]
async fn a_step_deadline_is_not_a_terminal_history_marker() {
    let (store, instance) = one_record_store().await;
    let deadline = StepDeadline::new(
        StepId::new("draft").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        StepAttempt::FIRST,
        StepClaimFence::new("0".repeat(64)).unwrap(),
        RoleId::new("writer").unwrap(),
        now(),
    );
    store
        .append(
            &protocol_id(),
            StreamVersion::new(1),
            vec![
                fact(
                    &instance,
                    "step-deadline",
                    ProtocolEvent::StepDeadlineExceeded(StepDeadlineExceeded {
                        deadline,
                        result: StepResult::timed_out().unwrap(),
                        observed_at: now(),
                    }),
                ),
                started_fact(&instance, "step-deadline-late"),
            ],
        )
        .await
        .unwrap();
    let usecase = StreamProtocolUseCase::new(store, Arc::new(TrackingNotifier::new(false)));

    let mut suffix = usecase.execute(input(2, 1, 0)).await.unwrap();
    assert!(matches!(
        suffix.next().await.unwrap().unwrap(),
        ProtocolProgressFrame::Record(record) if record.sequence().value() == 3
    ));
    let ProtocolProgressFrame::End(end) = suffix.next().await.unwrap().unwrap() else {
        panic!("step deadline suffix must finish with an end frame");
    };
    assert_eq!(end.reason(), ProtocolProgressEndReason::EventLimit);
}

fn lifecycle_terminal_events() -> Vec<(&'static str, ProtocolEvent)> {
    vec![
        (
            "cancelled",
            ProtocolEvent::ProtocolCancelled(ProtocolCancelled {
                reason: LifecycleReason::new("operator cancelled").unwrap(),
                cancelled_at: now(),
            }),
        ),
        (
            "protocol_deadline",
            ProtocolEvent::ProtocolDeadlineExceeded(ProtocolDeadlineExceeded {
                deadline: ProtocolDeadline::new(now()),
                observed_at: now(),
            }),
        ),
        (
            "state_deadline",
            ProtocolEvent::StateDeadlineExceeded(StateDeadlineExceeded {
                deadline: StateDeadline::new(
                    StateId::new("OPEN").unwrap(),
                    1.try_into().unwrap(),
                    now(),
                ),
                observed_at: now(),
            }),
        ),
    ]
}

#[derive(Debug)]
struct ActiveReadGuard(Arc<AtomicUsize>);

impl Drop for ActiveReadGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct BlockingReadStore {
    inner: Arc<EventStoreFake>,
    calls: AtomicUsize,
    active_reads: Arc<AtomicUsize>,
}

#[async_trait]
impl ProtocolEventStorePort for BlockingReadStore {
    async fn append(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= 2 {
            self.active_reads.fetch_add(1, Ordering::SeqCst);
            let _guard = ActiveReadGuard(self.active_reads.clone());
            futures::future::pending::<Result<Vec<AuditRecord>, DomainError>>().await
        } else {
            self.inner.read(stream, after, limit).await
        }
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

#[derive(Debug)]
struct TerminalReadRaceStore {
    inner: Arc<EventStoreFake>,
    instance: ProtocolInstance,
    raced: AtomicBool,
}

#[async_trait]
impl ProtocolEventStorePort for TerminalReadRaceStore {
    async fn append(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let records = self.inner.read(stream, after, limit).await?;
        if after == StreamVersion::new(1) && !self.raced.swap(true, Ordering::SeqCst) {
            self.inner
                .append(
                    stream,
                    StreamVersion::new(1),
                    vec![completed_fact(&self.instance, "terminal")],
                )
                .await?;
        }
        Ok(records)
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
