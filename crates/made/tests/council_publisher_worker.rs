//! Exercise the production polling loop before transport composition.
#[path = "../src/councils/council_publisher_worker.rs"]
mod council_publisher_worker;

use async_trait::async_trait;
use council_publisher_worker::CouncilPublisherWorker;
use made_adapters::clock::SystemClock;
use made_adapters::memory::InMemoryCouncilJournal;
use made_app::usecases::PublishCouncilEventsUseCase;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::events::{
    DeliberationCompletedEvent, EventEnvelope, PhaseChangedEvent, TaskCompletedEvent,
    TaskDispatchedEvent, TaskFailedEvent,
};
use made_core::ports::{CouncilJournalPort, MessagingPort};
use made_core::value_objects::{CouncilJournalConsumer, EventId, TaskId};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{watch, Notify};

struct RecoverableSink {
    calls: AtomicUsize,
    entered: Notify,
    hang: bool,
}
#[async_trait]
impl MessagingPort for RecoverableSink {
    async fn publish_phase_changed(&self, _: &PhaseChangedEvent) -> Result<(), DomainError> {
        let previous = self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.hang {
            std::future::pending::<()>().await;
        }
        if previous == 0 {
            Err(DomainError::InvariantViolated {
                reason: "transient transport failure",
            })
        } else {
            Ok(())
        }
    }
    async fn publish_task_dispatched(&self, _: &TaskDispatchedEvent) -> Result<(), DomainError> {
        unreachable!()
    }
    async fn publish_task_completed(&self, _: &TaskCompletedEvent) -> Result<(), DomainError> {
        unreachable!()
    }
    async fn publish_task_failed(&self, _: &TaskFailedEvent) -> Result<(), DomainError> {
        unreachable!()
    }
    async fn publish_deliberation_completed(
        &self,
        _: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        unreachable!()
    }
}
async fn fixture(
    hang: bool,
) -> (
    Arc<InMemoryCouncilJournal>,
    Arc<RecoverableSink>,
    CouncilJournalConsumer,
    CouncilPublisherWorker,
) {
    let journal = Arc::new(InMemoryCouncilJournal::new());
    let event = PhaseChangedEvent::new(
        EventEnvelope::new(
            EventId::new("pending-before-host-start").unwrap(),
            time::OffsetDateTime::now_utc(),
            "worker-fixture",
            None,
        )
        .unwrap(),
        TaskId::new("task-1").unwrap(),
        "proposing",
        "reviewing",
    )
    .unwrap();
    journal
        .publish(CouncilJournalEvent::PhaseChanged(event))
        .await
        .unwrap();
    let sink = Arc::new(RecoverableSink {
        calls: AtomicUsize::new(0),
        entered: Notify::new(),
        hang,
    });
    let consumer = CouncilJournalConsumer::new("worker-test").unwrap();
    let worker = CouncilPublisherWorker::new(
        Arc::new(PublishCouncilEventsUseCase::new(
            journal.clone(),
            sink.clone(),
            Arc::new(SystemClock::new()),
        )),
        consumer.clone(),
    );
    (journal, sink, consumer, worker)
}
#[tokio::test]
async fn periodic_recovery_retries_without_new_event_or_broker_notification() {
    let (journal, sink, consumer, worker) = fixture(false).await;
    let (stop, stopped) = watch::channel(false);
    let handle = tokio::spawn(worker.run(stopped));
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if journal.position(&consumer).await.unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("polling must retry the existing event on its own");
    assert_eq!(sink.calls.load(Ordering::SeqCst), 2);
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .unwrap()
        .unwrap();
}
#[tokio::test]
async fn cooperative_stop_during_delivery_does_not_acknowledge_pending_event() {
    let (journal, sink, consumer, worker) = fixture(true).await;
    let (stop, stopped) = watch::channel(false);
    let handle = tokio::spawn(worker.run(stopped));
    tokio::time::timeout(Duration::from_secs(1), sink.entered.notified())
        .await
        .unwrap();
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(journal.position(&consumer).await.unwrap().is_none());
    assert_eq!(sink.calls.load(Ordering::SeqCst), 1);
}
