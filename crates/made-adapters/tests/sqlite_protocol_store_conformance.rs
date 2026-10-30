//! The canonical embedded SQLite store against every persistence contract.
//!
//! Nothing but running the full contract proves that the durable adapter
//! preserves publication, event-stream and snapshot semantics — and that
//! what it wrote is still there after the process that wrote it is gone.

#![cfg(feature = "sqlite")]

use made_adapters::sqlite::SqliteProtocolStore;
use made_core::conformance::{
    ProtocolDefinitionPublicationConformance, ProtocolEventCursorConformance,
    ProtocolEventStoreConformance, ProtocolSnapshotStoreConformance,
};
use tempfile::TempDir;

/// Each suite gets its own database: several of them require a store
/// that nothing else has written to.
fn store() -> (TempDir, SqliteProtocolStore) {
    let directory = TempDir::new().expect("a temporary directory");
    let store = SqliteProtocolStore::open(directory.path().join("protocols.sqlite3"))
        .expect("the store opens");
    (directory, store)
}

#[tokio::test]
async fn sqlite_satisfies_the_publication_contract() {
    let (_directory, store) = store();

    let passed = ProtocolDefinitionPublicationConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 6, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_event_store_contract() {
    let (_directory, store) = store();

    let passed = ProtocolEventStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 14, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_event_cursor_contract() {
    let (_directory, store) = store();

    let passed = ProtocolEventCursorConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 7, "properties run: {passed:?}");
}

#[tokio::test]
async fn a_reopened_store_resumes_a_named_cursor() {
    use made_core::ports::ProtocolEventCursorPort;
    use made_core::value_objects::{GlobalPosition, ProtocolEventConsumer};

    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("cursor.sqlite3");
    let consumer = ProtocolEventConsumer::new("durable-consumer").unwrap();
    {
        let store = SqliteProtocolStore::open(&path).expect("the store opens");
        store
            .acknowledge(&consumer, GlobalPosition::new(17).unwrap())
            .await
            .unwrap();
    }

    let reopened = SqliteProtocolStore::open(&path).expect("the store reopens");
    assert_eq!(
        reopened.position(&consumer).await.unwrap(),
        Some(GlobalPosition::new(17).unwrap())
    );
}

#[tokio::test]
async fn sqlite_satisfies_the_snapshot_store_contract() {
    let (_directory, store) = store();

    let passed = ProtocolSnapshotStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 6, "properties run: {passed:?}");
}

/// The stream, its place in the global order and its snapshot all live
/// in the file, not in the process that wrote them — which is what no
/// in-memory adapter can be asked.
#[tokio::test]
async fn a_reopened_store_still_holds_its_streams_positions_and_snapshots() {
    use made_core::entities::AuditChain;
    use made_core::ports::{ProtocolEventStorePort, ProtocolSnapshot, ProtocolSnapshotStorePort};
    use made_core::value_objects::{
        GlobalPosition, ProtocolEventPageLimit, ProtocolId, StreamVersion,
    };

    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("protocols.sqlite3");
    let protocol = ProtocolId::new("survives-as-stream").unwrap();

    {
        let store = SqliteProtocolStore::open(&path).expect("the store opens");
        let facts = (1..=3)
            .map(|ordinal| support::fact(&protocol, ordinal))
            .collect();
        let outcome = store
            .append(&protocol, StreamVersion::EMPTY, facts)
            .await
            .unwrap();
        assert_eq!(outcome.appended_version(), Some(StreamVersion::new(3)));
        store
            .save(ProtocolSnapshot {
                version: StreamVersion::new(3),
                instance: support::instance(&protocol),
            })
            .await
            .unwrap();
    }

    let reopened = SqliteProtocolStore::open(&path).expect("the store reopens");

    assert_eq!(reopened.streams().await.unwrap(), vec![protocol.clone()]);
    assert_eq!(
        reopened.head(&protocol).await.unwrap(),
        StreamVersion::new(3),
        "the head did not survive reopening"
    );
    let records = reopened
        .read(
            &protocol,
            StreamVersion::EMPTY,
            ProtocolEventPageLimit::DEFAULT,
        )
        .await
        .unwrap();
    assert_eq!(records.len(), 3);
    assert!(
        AuditChain::verify(&records).is_intact(),
        "the stream did not survive reopening"
    );
    let positions: Vec<u64> = reopened
        .read_all(GlobalPosition::FIRST, ProtocolEventPageLimit::DEFAULT)
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
    let latest = reopened.latest(&protocol).await.unwrap();
    assert_eq!(
        latest.map(|snapshot| snapshot.version),
        Some(StreamVersion::new(3)),
        "the snapshot did not survive reopening"
    );
}

mod support {
    use made_core::entities::protocol_events::StepCompleted;
    use made_core::entities::ProtocolEvent;
    use made_core::entities::{AuditFact, ProtocolDefinition, ProtocolInstance};
    use made_core::value_objects::{
        AuditActor, AuditActorKind, EventId, ProtocolContext, ProtocolId, ProtocolName,
        ProtocolState, ProtocolTransition, ProtocolVersion, StateId, TransitionTrigger,
    };
    use made_core::value_objects::{
        RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult,
    };
    use time::OffsetDateTime;

    pub fn instance(protocol_id: &ProtocolId) -> ProtocolInstance {
        ProtocolInstance::start(
            protocol_id.clone(),
            &definition(),
            ProtocolContext::empty(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .expect("required protocol inputs")
    }

    pub fn fact(protocol_id: &ProtocolId, ordinal: u64) -> AuditFact {
        let definition = definition();
        AuditFact {
            event_id: EventId::new(format!("restart-{ordinal}")).unwrap(),
            event: ProtocolEvent::StepCompleted(StepCompleted {
                state_visit: None,
                step_id: StepId::new("conformance_step").unwrap(),
                state_iteration: None,
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                next_iteration: None,
                finished_by: RoleId::new("test").unwrap(),
                finished_at: OffsetDateTime::UNIX_EPOCH,
            }),
            protocol_id: protocol_id.clone(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    fn definition() -> ProtocolDefinition {
        ProtocolDefinition::new(
            ProtocolName::new("restart_protocol").unwrap(),
            ProtocolVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                ProtocolState::initial(StateId::new("OPEN").unwrap()),
                ProtocolState::terminal(StateId::new("DONE").unwrap()),
            ],
            vec![ProtocolTransition::new(
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
