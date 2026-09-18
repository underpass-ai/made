#![cfg(feature = "sqlite")]
use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::sqlite::{SqliteCouncilJournal, SqliteCouncilRegistry, SqliteCouncilStore};
use made_app::usecases::PublishCouncilEventsUseCase;
use made_core::entities::{Council, CouncilJournalEvent};
use made_core::error::DomainError;
use made_core::events::{
    DeliberationCompletedEvent, EventEnvelope, PhaseChangedEvent, TaskCompletedEvent,
    TaskDispatchedEvent, TaskFailedEvent,
};
use made_core::ports::{CouncilJournalPort, CouncilRegistryPort, MessagingPort};
use made_core::value_objects::{
    AgentId, CouncilId, CouncilJournalConsumer, CouncilJournalPageLimit, EventId, Specialty, TaskId,
};
use std::collections::BTreeSet;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use time::macros::datetime;

#[derive(Default)]
struct DeduplicatingSink {
    effects: Mutex<BTreeSet<String>>,
    fail_after_effect: AtomicBool,
    calls: AtomicUsize,
}
#[async_trait]
impl MessagingPort for DeduplicatingSink {
    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.effects
            .lock()
            .unwrap()
            .insert(event.envelope().event_id().as_str().to_owned());
        if self.fail_after_effect.swap(false, Ordering::SeqCst) {
            return Err(DomainError::InvariantViolated {
                reason: "delivery acknowledgement lost after effect",
            });
        }
        Ok(())
    }
    async fn publish_task_dispatched(&self, _: &TaskDispatchedEvent) -> Result<(), DomainError> {
        panic!("not part of this fixture")
    }
    async fn publish_task_completed(&self, _: &TaskCompletedEvent) -> Result<(), DomainError> {
        panic!("not part of this fixture")
    }
    async fn publish_task_failed(&self, _: &TaskFailedEvent) -> Result<(), DomainError> {
        panic!("not part of this fixture")
    }
    async fn publish_deliberation_completed(
        &self,
        _: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        panic!("not part of this fixture")
    }
}
#[tokio::test]
async fn lost_delivery_ack_reopens_and_concurrent_consumers_preserve_one_downstream_effect() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let path = directory.path().join("outbox.sqlite3");
    let store = SqliteCouncilStore::open(&path).unwrap();
    let now = datetime!(2026-09-19 00:00:00 UTC);
    SqliteCouncilRegistry::new(store.clone())
        .register(
            Council::new(
                CouncilId::new("research").unwrap(),
                Specialty::new("research").unwrap(),
                [AgentId::new("writer").unwrap()],
                now,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let journal = Arc::new(SqliteCouncilJournal::new(store));
    let event = CouncilJournalEvent::PhaseChanged(
        PhaseChangedEvent::new(
            EventEnvelope::new(
                EventId::new("stable-message").unwrap(),
                now,
                "fixture",
                None,
            )
            .unwrap(),
            TaskId::new("task-1").unwrap(),
            "proposing",
            "reviewing",
        )
        .unwrap(),
    );
    let pending = journal.publish(event.clone()).await.unwrap();
    assert_eq!(journal.publish(event).await.unwrap(), pending);
    let consumer = CouncilJournalConsumer::new("broker").unwrap();
    let sink = Arc::new(DeduplicatingSink {
        fail_after_effect: AtomicBool::new(true),
        ..Default::default()
    });
    let publisher = PublishCouncilEventsUseCase::new(
        journal.clone(),
        sink.clone(),
        Arc::new(SystemClock::new()),
    );
    assert!(publisher
        .execute(&consumer, CouncilJournalPageLimit::default())
        .await
        .is_err());
    assert_eq!(
        journal.position(&consumer).await.unwrap().unwrap().value(),
        1,
        "configuration fact skipped but unconfirmed bus event remains pending"
    );
    drop(publisher);
    drop(journal);
    let reopened = Arc::new(SqliteCouncilJournal::new(
        SqliteCouncilStore::open(&path).unwrap(),
    ));
    let publisher = PublishCouncilEventsUseCase::new(
        reopened.clone(),
        sink.clone(),
        Arc::new(SystemClock::new()),
    );
    let other = PublishCouncilEventsUseCase::new(
        Arc::new(SqliteCouncilJournal::new(
            SqliteCouncilStore::open(&path).unwrap(),
        )),
        sink.clone(),
        Arc::new(SystemClock::new()),
    );
    let (left, right) = tokio::join!(
        publisher.execute(&consumer, CouncilJournalPageLimit::default()),
        other.execute(&consumer, CouncilJournalPageLimit::default())
    );
    assert_eq!(left.unwrap().len() + right.unwrap().len(), 1);
    assert_eq!(
        reopened.position(&consumer).await.unwrap(),
        Some(pending.position())
    );
    assert_eq!(
        sink.calls.load(Ordering::SeqCst),
        2,
        "at-least-once, not exactly-once delivery"
    );
    assert_eq!(
        sink.effects.lock().unwrap().len(),
        1,
        "consumer deduplicates the stable original event ID"
    );
}
