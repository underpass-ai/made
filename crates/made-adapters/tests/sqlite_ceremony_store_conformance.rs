//! The canonical embedded SQLite store against every persistence contract.
//!
//! Nothing but running the full contract proves that the durable adapter
//! preserves publication, event-stream and snapshot semantics — and that
//! what it wrote is still there after the process that wrote it is gone.

#![cfg(feature = "sqlite")]

use made_adapters::sqlite::SqliteCeremonyStore;
use made_core::conformance::{
    CeremonyDefinitionPublicationConformance, CeremonyEventStoreConformance,
    CeremonySnapshotStoreConformance,
};
use tempfile::TempDir;

/// Each suite gets its own database: several of them require a store
/// that nothing else has written to.
fn store() -> (TempDir, SqliteCeremonyStore) {
    let directory = TempDir::new().expect("a temporary directory");
    let store = SqliteCeremonyStore::open(directory.path().join("ceremonies.sqlite3"))
        .expect("the store opens");
    (directory, store)
}

#[tokio::test]
async fn sqlite_satisfies_the_publication_contract() {
    let (_directory, store) = store();

    let passed = CeremonyDefinitionPublicationConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_event_store_contract() {
    let (_directory, store) = store();

    let passed = CeremonyEventStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 14, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_snapshot_store_contract() {
    let (_directory, store) = store();

    let passed = CeremonySnapshotStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}

/// The stream, its place in the global order and its snapshot all live
/// in the file, not in the process that wrote them — which is what no
/// in-memory adapter can be asked.
#[tokio::test]
async fn a_reopened_store_still_holds_its_streams_positions_and_snapshots() {
    use made_core::entities::AuditChain;
    use made_core::ports::{CeremonyEventStorePort, CeremonySnapshot, CeremonySnapshotStorePort};
    use made_core::value_objects::{CeremonyId, GlobalPosition, StreamVersion};

    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("ceremonies.sqlite3");
    let ceremony = CeremonyId::new("survives-as-stream").unwrap();

    {
        let store = SqliteCeremonyStore::open(&path).expect("the store opens");
        let facts = (1..=3)
            .map(|ordinal| support::fact(&ceremony, ordinal))
            .collect();
        let outcome = store
            .append(&ceremony, StreamVersion::EMPTY, facts)
            .await
            .unwrap();
        assert_eq!(outcome.appended_version(), Some(StreamVersion::new(3)));
        store
            .save(CeremonySnapshot {
                version: StreamVersion::new(3),
                instance: support::instance(&ceremony),
            })
            .await
            .unwrap();
    }

    let reopened = SqliteCeremonyStore::open(&path).expect("the store reopens");

    assert_eq!(reopened.streams().await.unwrap(), vec![ceremony.clone()]);
    assert_eq!(
        reopened.head(&ceremony).await.unwrap(),
        StreamVersion::new(3),
        "the head did not survive reopening"
    );
    let records = reopened
        .read(&ceremony, StreamVersion::EMPTY)
        .await
        .unwrap();
    assert_eq!(records.len(), 3);
    assert!(
        AuditChain::verify(&records).is_intact(),
        "the stream did not survive reopening"
    );
    let positions: Vec<u64> = reopened
        .read_all(GlobalPosition::FIRST, usize::MAX)
        .await
        .unwrap()
        .iter()
        .map(|row| row.position.value())
        .collect();
    assert_eq!(
        positions,
        [1, 2, 3],
        "the global order did not survive reopening"
    );
    let latest = reopened.latest(&ceremony).await.unwrap();
    assert_eq!(
        latest.map(|snapshot| snapshot.version),
        Some(StreamVersion::new(3)),
        "the snapshot did not survive reopening"
    );
}

mod support {
    use made_core::entities::ceremony_events::StepCompleted;
    use made_core::entities::CeremonyEvent;
    use made_core::entities::{AuditFact, CeremonyDefinition, CeremonyInstance};
    use made_core::value_objects::{
        AuditActor, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyState,
        CeremonyTransition, CeremonyVersion, EventId, StateId, TransitionTrigger,
    };
    use made_core::value_objects::{
        RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult,
    };
    use time::OffsetDateTime;

    pub fn instance(ceremony_id: &CeremonyId) -> CeremonyInstance {
        CeremonyInstance::start(
            ceremony_id.clone(),
            &definition(),
            CeremonyContext::empty(),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    pub fn fact(ceremony_id: &CeremonyId, ordinal: u64) -> AuditFact {
        let definition = definition();
        AuditFact {
            event_id: EventId::new(format!("restart-{ordinal}")).unwrap(),
            event: CeremonyEvent::StepCompleted(StepCompleted {
                step_id: StepId::new("conformance_step").unwrap(),
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                next_iteration: None,
                finished_by: RoleId::new("test").unwrap(),
                finished_at: OffsetDateTime::UNIX_EPOCH,
            }),
            ceremony_id: ceremony_id.clone(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    fn definition() -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new("restart_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(StateId::new("OPEN").unwrap()),
                CeremonyState::terminal(StateId::new("DONE").unwrap()),
            ],
            vec![CeremonyTransition::new(
                StateId::new("OPEN").unwrap(),
                StateId::new("DONE").unwrap(),
                TransitionTrigger::new("finish").unwrap(),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }
}
