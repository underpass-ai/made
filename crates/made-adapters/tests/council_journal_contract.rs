#![cfg(feature = "sqlite")]
use made_adapters::memory::InMemoryCouncilJournal;
use made_adapters::sqlite::{SqliteCouncilJournal, SqliteCouncilStore};
use made_core::entities::CouncilJournalEvent;
use made_core::events::{EventEnvelope, PhaseChangedEvent};
use made_core::ports::CouncilJournalPort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationEvidence, AuthorizedOperation,
    CouncilJournalConsumer, CouncilJournalPageLimit, CouncilJournalPosition, DurationMs, EventId,
    PrincipalId, PrincipalKind, TaskId,
};
use time::macros::datetime;

fn publication(id: &str, phase: &str) -> CouncilJournalEvent {
    let now = datetime!(2026-09-19 00:00:00 UTC);
    CouncilJournalEvent::PhaseChanged(
        PhaseChangedEvent::new(
            EventEnvelope::new(EventId::new(id).unwrap(), now, "fixture", None).unwrap(),
            TaskId::new("task-1").unwrap(),
            "proposing",
            phase,
        )
        .unwrap(),
    )
}

async fn conformance(journal: &dyn CouncilJournalPort) {
    let now = datetime!(2026-09-19 00:00:00 UTC);
    let event = CouncilJournalEvent::PhaseChanged(
        PhaseChangedEvent::new(
            EventEnvelope::new(EventId::new("phase-1").unwrap(), now, "fixture", None).unwrap(),
            TaskId::new("task-1").unwrap(),
            "proposing",
            "reviewing",
        )
        .unwrap(),
    );
    let first = journal.publish(event.clone()).await.unwrap();
    assert_eq!(journal.publish(event).await.unwrap(), first);
    let consumer = CouncilJournalConsumer::new("reader").unwrap();
    let lease = journal
        .lease(&consumer, now, DurationMs::from_millis(1000))
        .await
        .unwrap()
        .unwrap();
    assert!(journal
        .lease(&consumer, now, DurationMs::from_millis(1000))
        .await
        .unwrap()
        .is_none());
    let later = now + time::Duration::seconds(2);
    let current = journal
        .lease(&consumer, later, DurationMs::from_millis(1000))
        .await
        .unwrap()
        .unwrap();
    assert!(journal.release(&lease, later).await.is_err());
    assert!(journal
        .acknowledge(&lease, first.position(), later)
        .await
        .is_err());
    assert!(journal
        .acknowledge(&current, CouncilJournalPosition::new(2).unwrap(), later)
        .await
        .is_err());
    journal
        .acknowledge(&current, first.position(), later)
        .await
        .unwrap();
    assert_eq!(
        journal.position(&consumer).await.unwrap(),
        Some(first.position())
    );
    assert!(journal
        .read(Some(first.position()), CouncilJournalPageLimit::default())
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        journal
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        vec![first]
    );
}

fn authorized_operation() -> AuthorizedOperation {
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("council-worker").unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap();
    let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
        "decision_id": "a".repeat(64),
        "request_id": "council-publication-request",
        "principal_id": "council-worker",
        "action": "process_trigger_event",
        "scope": { "kind": "global" },
        "target_digest": "b".repeat(64),
        "policy_version": 1,
        "admitted_at": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T12:01:00Z"
    }))
    .unwrap();
    AuthorizedOperation::new(principal, evidence).unwrap()
}

async fn authorization_conformance(journal: &dyn CouncilJournalPort) {
    let event = publication("authorized-phase", "reviewing");
    let first = journal
        .publish_authorized(
            event.clone(),
            Some(authorized_operation().evidence().clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        first.authorization().unwrap().principal_id().as_str(),
        "council-worker"
    );
    assert_eq!(journal.publish(event).await.unwrap(), first);
    assert_eq!(
        journal
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        vec![first]
    );
}
#[tokio::test]
async fn memory_and_sqlite_obey_the_same_publication_and_fencing_contract() {
    conformance(&InMemoryCouncilJournal::new()).await;
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    conformance(&SqliteCouncilJournal::new(
        SqliteCouncilStore::open(directory.path().join("contract.sqlite3")).unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn memory_and_sqlite_preserve_the_first_authorized_publication() {
    authorization_conformance(&InMemoryCouncilJournal::new()).await;
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    authorization_conformance(&SqliteCouncilJournal::new(
        SqliteCouncilStore::open(directory.path().join("authorization.sqlite3")).unwrap(),
    ))
    .await;
}
