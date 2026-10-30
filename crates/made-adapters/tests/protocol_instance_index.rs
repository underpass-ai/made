use made_adapters::memory::InMemoryProtocolEventStore;
use made_adapters::sqlite::SqliteProtocolStore;
use made_core::entities::protocol_events::ProtocolCompleted;
use made_core::entities::{AuditFact, ProtocolEvent};
use made_core::ports::{ProtocolEventStorePort, ProtocolInstanceIndexPort};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, ProtocolId, ProtocolIdPrefix, ProtocolInstancePageLimit,
    ProtocolName, ProtocolVersion, StateId, StreamVersion,
};
use time::OffsetDateTime;

#[tokio::test]
async fn memory_index_is_keyset_ordered_and_prefix_filtered() {
    let store = InMemoryProtocolEventStore::new();
    exercise(&store).await;
}

#[tokio::test]
async fn sqlite_index_survives_reopen() {
    let scratch = scratch();
    let path = scratch.path().join("protocols.sqlite3");
    {
        let store = SqliteProtocolStore::open(&path).unwrap();
        exercise(&store).await;
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM protocol_stream_index", [])
        .unwrap();
    connection
        .execute(
            "DELETE FROM store_meta WHERE k = 'protocol_stream_index_backfilled_v1'",
            [],
        )
        .unwrap();
    let transaction = connection.unchecked_transaction().unwrap();
    for ordinal in 0..1_100_u64 {
        let id = format!("bulk-{ordinal:04}");
        transaction
            .execute(
                "INSERT INTO protocol_events (k, v) VALUES (?1, ?2)",
                rusqlite::params![event_key(id.as_bytes(), 1), b"payload-not-read"],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);

    let reopened = SqliteProtocolStore::open(path).unwrap();
    let page = reopened
        .ids_after(
            None,
            Some(&ProtocolIdPrefix::new("team-").unwrap()),
            ProtocolInstancePageLimit::new(10).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        page.ids()
            .iter()
            .map(ProtocolId::as_str)
            .collect::<Vec<_>>(),
        ["team-a", "team-aa", "team-b"]
    );
    assert!(!page.has_more());

    let prefix = ProtocolIdPrefix::new("bulk-").unwrap();
    let mut after = None;
    let mut count = 0;
    loop {
        let page = reopened
            .ids_after(
                after.as_ref(),
                Some(&prefix),
                ProtocolInstancePageLimit::new(100).unwrap(),
            )
            .await
            .unwrap();
        count += page.ids().len();
        if !page.has_more() {
            break;
        }
        after = page.ids().last().cloned();
    }
    assert_eq!(count, 1_100);
}

#[test]
fn sqlite_backfill_rejects_invalid_utf8_without_installing_marker_or_index() {
    let scratch = scratch();
    let path = scratch.path().join("corrupt.sqlite3");
    drop(SqliteProtocolStore::open(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM protocol_stream_index", [])
        .unwrap();
    connection
        .execute(
            "DELETE FROM store_meta WHERE k = 'protocol_stream_index_backfilled_v1'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO protocol_events (k, v) VALUES (?1, ?2)",
            rusqlite::params![event_key(&[0xff], 1), b"payload-not-read"],
        )
        .unwrap();
    drop(connection);

    assert!(SqliteProtocolStore::open(&path).is_err());

    let connection = rusqlite::Connection::open(path).unwrap();
    let index_rows: i64 = connection
        .query_row("SELECT count(*) FROM protocol_stream_index", [], |row| {
            row.get(0)
        })
        .unwrap();
    let marker_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM store_meta WHERE k = \
             'protocol_stream_index_backfilled_v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(index_rows, 0);
    assert_eq!(marker_rows, 0);
}

async fn exercise<T>(store: &T)
where
    T: ProtocolEventStorePort + ProtocolInstanceIndexPort,
{
    for id in ["other-a", "team-b", "team-a"] {
        let protocol_id = ProtocolId::new(id).unwrap();
        store
            .append(
                &protocol_id,
                StreamVersion::EMPTY,
                vec![fact(protocol_id.clone(), id)],
            )
            .await
            .unwrap();
    }
    let prefix = ProtocolIdPrefix::new("team-").unwrap();
    let first = store
        .ids_after(
            None,
            Some(&prefix),
            ProtocolInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.ids()[0].as_str(), "team-a");
    assert!(first.has_more());
    let inserted = ProtocolId::new("team-aa").unwrap();
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
            ProtocolInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.ids()[0].as_str(), "team-aa");
    assert!(second.has_more());
    let third = store
        .ids_after(
            second.ids().first(),
            Some(&prefix),
            ProtocolInstancePageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(third.ids()[0].as_str(), "team-b");
    assert!(!third.has_more());
}

fn fact(protocol_id: ProtocolId, suffix: &str) -> AuditFact {
    AuditFact {
        event_id: EventId::new(format!("index-{suffix}")).unwrap(),
        event: ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id,
        definition_name: ProtocolName::new("index_test").unwrap(),
        definition_version: ProtocolVersion::v1(),
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

fn event_key(id: &[u8], sequence: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(id.len() + 9);
    key.extend_from_slice(id);
    key.push(0);
    key.extend_from_slice(&sequence.to_be_bytes());
    key
}
