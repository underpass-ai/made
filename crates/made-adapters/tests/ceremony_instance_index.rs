use made_adapters::memory::InMemoryCeremonyEventStore;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_core::entities::ceremony_events::CeremonyCompleted;
use made_core::entities::{AuditFact, CeremonyEvent};
use made_core::ports::{CeremonyEventStorePort, CeremonyInstanceIndexPort};
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyId, CeremonyIdPrefix, CeremonyInstancePageLimit,
    CeremonyName, CeremonyVersion, EventId, StateId, StreamVersion,
};
use time::OffsetDateTime;

#[tokio::test]
async fn memory_index_is_keyset_ordered_and_prefix_filtered() {
    let store = InMemoryCeremonyEventStore::new();
    exercise(&store).await;
}

#[tokio::test]
async fn sqlite_index_survives_reopen() {
    let scratch = scratch();
    let path = scratch.path().join("ceremonies.sqlite3");
    {
        let store = SqliteCeremonyStore::open(&path).unwrap();
        exercise(&store).await;
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM ceremony_stream_index", [])
        .unwrap();
    connection
        .execute(
            "DELETE FROM store_meta WHERE k = 'ceremony_stream_index_backfilled_v1'",
            [],
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteCeremonyStore::open(path).unwrap();
    let page = reopened
        .ids_after(
            None,
            Some(&CeremonyIdPrefix::new("team-").unwrap()),
            CeremonyInstancePageLimit::new(10).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        page.ids()
            .iter()
            .map(CeremonyId::as_str)
            .collect::<Vec<_>>(),
        ["team-a", "team-aa", "team-b"]
    );
    assert!(!page.has_more());
}

async fn exercise<T>(store: &T)
where
    T: CeremonyEventStorePort + CeremonyInstanceIndexPort,
{
    for id in ["other-a", "team-b", "team-a"] {
        let ceremony_id = CeremonyId::new(id).unwrap();
        store
            .append(
                &ceremony_id,
                StreamVersion::EMPTY,
                vec![fact(ceremony_id.clone(), id)],
            )
            .await
            .unwrap();
    }
    let prefix = CeremonyIdPrefix::new("team-").unwrap();
    let first = store
        .ids_after(
            None,
            Some(&prefix),
            CeremonyInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.ids()[0].as_str(), "team-a");
    assert!(first.has_more());
    let inserted = CeremonyId::new("team-aa").unwrap();
    store
        .append(
            &inserted,
            StreamVersion::EMPTY,
            vec![fact(inserted.clone(), "team-aa")],
        )
        .await
        .unwrap();
    let second = store
        .ids_after(
            first.ids().first(),
            Some(&prefix),
            CeremonyInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.ids()[0].as_str(), "team-aa");
    assert!(second.has_more());
    let third = store
        .ids_after(
            second.ids().first(),
            Some(&prefix),
            CeremonyInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(third.ids()[0].as_str(), "team-b");
    assert!(!third.has_more());
}

fn fact(ceremony_id: CeremonyId, suffix: &str) -> AuditFact {
    AuditFact {
        event_id: EventId::new(format!("index-{suffix}")).unwrap(),
        event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        ceremony_id,
        definition_name: CeremonyName::new("index_test").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("index-test", AuditActorKind::Service, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
