//! Actual overlapping workers against one SQLite journal, with deterministic lease time.
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::services::SessionStream;
use made_app::usecases::{
    CompleteCeremonyStepInput, RunCeremonyStepInput, StartCeremonyInput, StartCeremonyStepInput,
};
use made_core::entities::AuditChain;
use made_core::ports::{
    CeremonySnapshot, CeremonySnapshotStorePort, ClockPort, NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActorKind, AuditEventType, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey,
    LeaseOwnerId, RoleId, StepClaimFence, StepErrorMessage, StepId, StepOutput, StepResult,
    StepStatus,
};
use made_embedded::EmbeddedMade;
use time::OffsetDateTime;
use tokio::sync::Notify;

const DEFINITION: &str = r#"
version: "1.0"
name: claim_fencing
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: host_callback
roles:
  - id: WORKER
    allowed_actions: [work, finish]
retry_policies:
  default:
    max_attempts: 3
    backoff_seconds: 0
"#;

#[derive(Default)]
struct ControlledClock(AtomicI64);
impl ClockPort for ControlledClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(self.0.load(Ordering::SeqCst)).unwrap()
    }
}
fn id() -> CeremonyId {
    CeremonyId::new("fencing-race").unwrap()
}
fn step() -> StepId {
    StepId::new("work").unwrap()
}
fn role() -> RoleId {
    RoleId::new("WORKER").unwrap()
}
fn claim_input(owner: &str) -> StartCeremonyStepInput {
    StartCeremonyStepInput::new(
        id(),
        role(),
        AuditActorKind::Agent,
        step(),
        LeaseOwnerId::new(owner).unwrap(),
        IdempotencyKey::new(owner).unwrap(),
        DurationMs::from_millis(1000),
    )
}
fn run_input(owner: &str) -> RunCeremonyStepInput {
    RunCeremonyStepInput::new(
        id(),
        role(),
        AuditActorKind::Agent,
        step(),
        LeaseOwnerId::new(owner).unwrap(),
        IdempotencyKey::new(owner).unwrap(),
        DurationMs::from_millis(1000),
    )
}
fn completion(fence: &StepClaimFence) -> CompleteCeremonyStepInput {
    CompleteCeremonyStepInput::new(
        id(),
        step(),
        StepResult::completed(StepOutput::empty()).unwrap(),
        AuditActorKind::Agent,
        fence.clone(),
    )
}
async fn mount(engine: &EmbeddedMade) {
    engine.mount_yaml(DEFINITION).await.unwrap();
}
async fn start(engine: &EmbeddedMade) {
    let definition = engine.mount_yaml(DEFINITION).await.unwrap().definitions()[0].clone();
    engine
        .start(StartCeremonyInput::new(
            id(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
}
fn temporary_store() -> (tempfile::TempDir, Arc<SqliteCeremonyStore>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/claim-fencing-127");
    std::fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(dir.path().join("race.sqlite3")).unwrap());
    (dir, store)
}
fn stream(store: Arc<SqliteCeremonyStore>) -> SessionStream {
    SessionStream::new(store.clone(), store, Arc::new(NoopCeremonyEventSubscriber))
}
async fn assert_unchanged(
    engine: &EmbeddedMade,
    before: &[made_core::entities::AuditRecord],
    fence: &StepClaimFence,
) {
    assert_eq!(
        engine.audit_records(&id()).await.unwrap(),
        before,
        "stale A appended a fact"
    );
    let current = engine.instance(&id()).await.unwrap();
    assert_eq!(&current.step_claim_fence(&step()).unwrap(), fence);
    let record = current.step_record(&step()).unwrap();
    assert_eq!(record.status(), StepStatus::InProgress);
    assert_eq!(record.attempt().get(), 2);
    assert_eq!(record.lease().unwrap().owner_id().as_str(), "worker-b");
}
async fn assert_reopen(
    engine: &EmbeddedMade,
    store: &SqliteCeremonyStore,
    path: &std::path::Path,
    midpoint: CeremonySnapshot,
) {
    let records = engine.audit_records(&id()).await.unwrap();
    assert!(AuditChain::verify(&records).is_intact());
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(
                r.event_type(),
                AuditEventType::StepCompleted | AuditEventType::StepFailed
            ))
            .count(),
        1
    );
    let full = SessionStream::fold_records(&records).unwrap();
    assert_eq!(
        full.instance.step_record(&step()).unwrap().attempt().get(),
        2
    );
    // Deliberately retain the replacement claim snapshot: reopening must fold B's ending as tail.
    store.forget(&id()).await.unwrap();
    store.save(midpoint).await.unwrap();
    let reopened = Arc::new(SqliteCeremonyStore::open(path).unwrap());
    let from_snapshot = stream(reopened.clone()).load(&id()).await.unwrap();
    assert_eq!(from_snapshot.instance, full.instance);
    assert_eq!(from_snapshot.version, full.version);
    reopened.forget(&id()).await.unwrap();
    let from_stream = stream(reopened).load(&id()).await.unwrap();
    assert_eq!(from_stream.instance, full.instance);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delegated_late_worker_cannot_finish_or_clear_replacement_lease() {
    let (dir, store) = temporary_store();
    let clock = Arc::new(ControlledClock::default());
    let a = EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_clock(clock.clone())
        .build();
    // Separate handles and engines share the actual SQLite journal.
    let path = dir.path().join("race.sqlite3");
    let b = EmbeddedMade::builder()
        .with_ceremony_store(Arc::new(SqliteCeremonyStore::open(&path).unwrap()))
        .with_clock(clock.clone())
        .build();
    start(&a).await;
    mount(&b).await;
    let claimed_a = Arc::new(Notify::new());
    let release_a = Arc::new(Notify::new());
    let worker_a = {
        let claimed = claimed_a.clone();
        let release = release_a.clone();
        tokio::spawn(async move {
            let claim = a.start_step(claim_input("worker-a")).await.unwrap();
            claimed.notify_one();
            release.notified().await;
            a.complete_step(completion(claim.claim_fence())).await
        })
    };
    claimed_a.notified().await;
    clock.0.store(2, Ordering::SeqCst);
    let claim_b = b.start_step(claim_input("worker-b")).await.unwrap();
    let before = b.audit_records(&id()).await.unwrap();
    let midpoint = store.latest(&id()).await.unwrap().unwrap();
    release_a.notify_one();
    assert!(worker_a
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("claim fence"));
    assert_unchanged(&b, &before, claim_b.claim_fence()).await;
    // A forged/misrouted well-formed identity also cannot touch B's claim.
    assert!(b
        .complete_step(completion(&StepClaimFence::new("0".repeat(64)).unwrap()))
        .await
        .is_err());
    assert_unchanged(&b, &before, claim_b.claim_fence()).await;
    let accepted = b
        .complete_step(completion(claim_b.claim_fence()))
        .await
        .unwrap();
    let sealed = b.audit_records(&id()).await.unwrap();
    assert_eq!(
        b.complete_step(completion(claim_b.claim_fence()))
            .await
            .unwrap(),
        accepted
    );
    assert_eq!(b.audit_records(&id()).await.unwrap(), sealed);
    assert_reopen(&b, &store, &path, midpoint).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_owned_late_handler_cannot_rebind_to_replacement_claim() {
    let (dir, store) = temporary_store();
    let clock = Arc::new(ControlledClock::default());
    let entered_a = Arc::new(Notify::new());
    let release_a = Arc::new(Notify::new());
    let entered_b = Arc::new(Notify::new());
    let release_b = Arc::new(Notify::new());
    let a = EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_clock(clock.clone())
        .with_step_handler_callback({
            let entered = entered_a.clone();
            let release = release_a.clone();
            move |_| {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    StepResult::failed(StepErrorMessage::new("late A failure").unwrap())
                }
            }
        })
        .build();
    let path = dir.path().join("race.sqlite3");
    let b = EmbeddedMade::builder()
        .with_ceremony_store(Arc::new(SqliteCeremonyStore::open(&path).unwrap()))
        .with_clock(clock.clone())
        .with_step_handler_callback({
            let entered = entered_b.clone();
            let release = release_b.clone();
            move |_| {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    StepResult::completed(StepOutput::empty())
                }
            }
        })
        .build();
    start(&a).await;
    mount(&b).await;
    let worker_a = tokio::spawn(async move { Box::pin(a.run_step(run_input("worker-a"))).await });
    entered_a.notified().await;
    clock.0.store(2, Ordering::SeqCst);
    let worker_b = {
        let engine = b.clone();
        tokio::spawn(async move { Box::pin(engine.run_step(run_input("worker-b"))).await })
    };
    entered_b.notified().await;
    let before = b.audit_records(&id()).await.unwrap();
    let claim_b = b
        .instance(&id())
        .await
        .unwrap()
        .step_claim_fence(&step())
        .unwrap();
    let midpoint = store.latest(&id()).await.unwrap().unwrap();
    release_a.notify_one();
    assert!(worker_a
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("claim fence"));
    assert_unchanged(&b, &before, &claim_b).await;
    release_b.notify_one();
    let output = worker_b.await.unwrap().unwrap();
    assert_eq!(output.attempt().get(), 2);
    assert!(output.result().is_success());
    let accepted = b.instance(&id()).await.unwrap();
    let sealed = b.audit_records(&id()).await.unwrap();
    assert_eq!(
        b.complete_step(completion(&claim_b)).await.unwrap(),
        accepted
    );
    assert_eq!(b.audit_records(&id()).await.unwrap(), sealed);
    assert_reopen(&b, &store, &path, midpoint).await;
}
