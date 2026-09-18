//! Open actual v0.6.0 data and continue it without rewriting its sealed prefix.

use std::path::{Path, PathBuf};

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::usecases::StartCeremonyStepInput;
use made_core::entities::{AuditChain, AuditRecord, CeremonyInstance};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    AuditActorKind, CeremonyEventPageLimit, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId,
    RoleId, StepId, StreamVersion,
};
use made_embedded::EmbeddedMade;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/stores/v0.6.0")
}

fn copy_fixture() -> (tempfile::TempDir, PathBuf) {
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let path = directory.path().join("ceremonies.sqlite3");
    std::fs::copy(fixture().join("ceremonies.sqlite3"), &path).unwrap();
    (directory, path)
}

fn id(value: &str) -> CeremonyId {
    CeremonyId::new(value).unwrap()
}

fn expected_records(session: &str) -> Vec<AuditRecord> {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/stores/v0.6.0/manifest.json")).unwrap();
    serde_json::from_value(manifest["sessions"][session]["records"].clone()).unwrap()
}

async fn records(store: &SqliteCeremonyStore, session: &str) -> Vec<AuditRecord> {
    store
        .read(
            &id(session),
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::DEFAULT,
        )
        .await
        .unwrap()
}

fn next_claim(session: &str) -> StartCeremonyStepInput {
    StartCeremonyStepInput::new(
        id(session),
        RoleId::new("WORKER").unwrap(),
        AuditActorKind::Agent,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new("upgraded-writer").unwrap(),
        IdempotencyKey::new("after-upgrade").unwrap(),
        DurationMs::from_millis(30_000),
    )
}

#[tokio::test]
async fn released_streams_keep_their_records_hashes_and_fold_after_upgrade() {
    let (_directory, path) = copy_fixture();
    let made = EmbeddedMade::open(&path).unwrap();
    let store = SqliteCeremonyStore::open(&path).unwrap();
    for session in ["v060-completed", "v060-pending", "v060-claimed"] {
        let actual = records(&store, session).await;
        let historical = expected_records(session);
        assert_eq!(actual, historical, "{session}: a historical record changed");
        assert!(AuditChain::verify(&actual).is_intact());
        let folded =
            CeremonyInstance::rehydrate(actual.iter().map(|r| r.event().unwrap())).unwrap();
        assert_eq!(made.instance(&id(session)).await.unwrap(), folded);
        assert_eq!(folded.completed_at().is_some(), session == "v060-completed");
    }
    drop(made);
    drop(store);
    let reopened = SqliteCeremonyStore::open(&path).unwrap();
    for session in ["v060-completed", "v060-pending", "v060-claimed"] {
        assert_eq!(records(&reopened, session).await, expected_records(session));
    }
}

#[tokio::test]
async fn upgraded_writer_continues_pending_data_and_keeps_completed_data_terminal() {
    let (_directory, path) = copy_fixture();
    let made = EmbeddedMade::open(&path).unwrap();
    made.start_step(next_claim("v060-pending")).await.unwrap();
    assert!(made.start_step(next_claim("v060-completed")).await.is_err());
    drop(made);
    let store = SqliteCeremonyStore::open(&path).unwrap();
    let pending = records(&store, "v060-pending").await;
    let historical = expected_records("v060-pending");
    assert_eq!(&pending[..historical.len()], historical.as_slice());
    assert_eq!(pending.len(), historical.len() + 1);
    assert!(AuditChain::verify(&pending).is_intact());
    assert_eq!(
        records(&store, "v060-completed").await,
        expected_records("v060-completed")
    );
    let reopened = EmbeddedMade::open(&path).unwrap();
    assert_eq!(
        reopened.instance(&id("v060-pending")).await.unwrap(),
        CeremonyInstance::rehydrate(pending.iter().map(|r| r.event().unwrap())).unwrap()
    );
}
