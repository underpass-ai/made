//! `made-mcp migrate-store` on a store MADE v0.3.1 really wrote.
//!
//! The fixture under `fixtures/stores/v0.3.1/` was produced by the
//! v0.3.1 binary over stdio (see its README). Everything here is a
//! claim about that file: that both of its sessions come into streams,
//! that each stream folds back to the session it imported and verifies
//! as a chain, that the mid-flight one accepts its next claim
//! afterwards, that the original is kept untouched beside the new
//! file, and that a second run changes nothing.

use std::path::{Path, PathBuf};

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::usecases::StartCeremonyStepInput;
use made_core::entities::{AuditChain, CeremonyEvent};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    AuditActorKind, AuditEventType, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId,
    StateId, StepId, StreamVersion,
};
use made_embedded::EmbeddedMade;
use made_mcp::MigrateStoreOutcome;

const FIXTURE: &str = "fixtures/stores/v0.3.1/ceremonies.sqlite3";
const COMPLETE: &str = "pre-stream-complete";
const MIDFLIGHT: &str = "pre-stream-midflight";

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

/// A copy of the fixture in a directory of its own. The committed file
/// is evidence; a test that migrated it in place would spend it.
fn a_copy_of_the_fixture(directory: &Path) -> PathBuf {
    let path = directory.join("ceremonies.sqlite3");
    std::fs::copy(fixture_path(), &path).expect("the fixture copies");
    path
}

fn ceremony(id: &str) -> CeremonyId {
    CeremonyId::new(id).unwrap()
}

#[tokio::test]
async fn every_pre_stream_session_is_imported_and_folds_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = a_copy_of_the_fixture(directory.path());

    let outcome = made_mcp::migrate_store(&path).await.expect("the migration");

    let MigrateStoreOutcome::Migrated { imported, backup } = outcome else {
        panic!("a store from v0.3.1 has sessions to import");
    };
    assert_eq!(imported, [ceremony(COMPLETE), ceremony(MIDFLIGHT)]);

    // The original is beside the new file, byte for byte.
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        std::fs::read(fixture_path()).unwrap(),
        "the pre-stream store must be kept exactly as it was"
    );

    let store = SqliteCeremonyStore::open(&path).expect("the migrated store opens");
    assert_eq!(
        store.streams().await.unwrap(),
        [ceremony(COMPLETE), ceremony(MIDFLIGHT)]
    );
    for id in [COMPLETE, MIDFLIGHT] {
        let records = store
            .read(&ceremony(id), StreamVersion::EMPTY)
            .await
            .unwrap();

        assert_eq!(records.len(), 1, "{id}: an import opens a stream of one");
        assert_eq!(records[0].event_type(), AuditEventType::InstanceImported);
        assert!(
            AuditChain::verify(&records).is_intact(),
            "{id}: the migrated stream does not verify"
        );

        let Some(CeremonyEvent::InstanceImported(imported)) = records[0].event() else {
            panic!("{id}: the genesis record carries the import");
        };
        // The legacy journal had records, and the import says which
        // one it ended on.
        assert!(
            imported.legacy_journal_head_hash.is_some(),
            "{id}: the legacy journal head is provenance the import must carry"
        );
        assert_eq!(imported.snapshot.id(), &ceremony(id));
    }
}

#[tokio::test]
async fn the_imported_sessions_are_the_ones_the_old_store_held() {
    let directory = tempfile::tempdir().unwrap();
    let path = a_copy_of_the_fixture(directory.path());
    made_mcp::migrate_store(&path).await.expect("the migration");

    let made = EmbeddedMade::open(&path).expect("the migrated store opens as an engine");

    let completed = made.instance(&ceremony(COMPLETE)).await.unwrap();
    assert_eq!(completed.current_state(), &StateId::new("CLOSED").unwrap());
    assert!(completed.completed_at().is_some());
    assert!(
        completed.bound_definition().is_some(),
        "the definition binding has to survive the import"
    );

    let midflight = made.instance(&ceremony(MIDFLIGHT)).await.unwrap();
    assert_eq!(
        midflight.current_state(),
        &StateId::new("SETTLING").unwrap()
    );
    assert_eq!(
        midflight.interventions().len(),
        1,
        "the open intervention has to survive the import"
    );
    assert!(midflight.completed_at().is_none());
}

/// The point of the whole migration: a session that was stuck can be
/// driven again.
#[tokio::test]
async fn the_mid_flight_session_accepts_its_next_claim_after_the_import() {
    let directory = tempfile::tempdir().unwrap();
    let path = a_copy_of_the_fixture(directory.path());
    made_mcp::migrate_store(&path).await.expect("the migration");
    let made = EmbeddedMade::open(&path).expect("the migrated store opens as an engine");

    made.start_step(StartCeremonyStepInput::new(
        ceremony(MIDFLIGHT),
        RoleId::new("RECORDER").unwrap(),
        AuditActorKind::Human,
        StepId::new("settle").unwrap(),
        LeaseOwnerId::new("operator").unwrap(),
        IdempotencyKey::new("after-the-import").unwrap(),
        DurationMs::from_millis(300_000),
    ))
    .await
    .expect("the imported session takes its next step");

    let store = SqliteCeremonyStore::open(&path).expect("the store reopens");
    let records = store
        .read(&ceremony(MIDFLIGHT), StreamVersion::EMPTY)
        .await
        .unwrap();
    assert_eq!(
        records
            .iter()
            .map(made_core::entities::AuditRecord::event_type)
            .collect::<Vec<_>>(),
        [
            AuditEventType::InstanceImported,
            AuditEventType::StepStarted
        ],
        "the claim continues the stream the import opened"
    );
    assert!(
        AuditChain::verify(&records).is_intact(),
        "a claim after an import must chain onto the genesis record"
    );
}

#[tokio::test]
async fn a_second_run_changes_nothing_and_says_so() {
    let directory = tempfile::tempdir().unwrap();
    let path = a_copy_of_the_fixture(directory.path());
    made_mcp::migrate_store(&path).await.expect("the migration");
    let after_the_first = std::fs::read(&path).unwrap();

    let outcome = made_mcp::migrate_store(&path)
        .await
        .expect("the second run");

    assert_eq!(outcome, MigrateStoreOutcome::AlreadyMigrated);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        after_the_first,
        "a second run must not rewrite the store"
    );
    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        2,
        "only the store and the original it kept"
    );
}
