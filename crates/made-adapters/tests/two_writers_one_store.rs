//! Two OS processes open the canonical SQLite store and both work.

#![cfg(feature = "sqlite")]

use std::path::Path;
use std::process::{Command, Stdio};

use made_adapters::sqlite::SqliteCeremonyStore;
use made_core::entities::AuditChain;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion};
use tempfile::TempDir;

const EVENTS_PER_WRITER: u64 = 40;

fn spawn(path: &Path, ceremony: &str) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_store_writer"))
        .arg(path)
        .arg(ceremony)
        .arg(EVENTS_PER_WRITER.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the writer spawns")
}

/// Runs two writers concurrently on one store and reports how many finished,
/// carrying the losers' stderr with the count.
///
/// The stderr matters: "1 != 2" is not a diagnosis, and a concurrency test
/// that fails without saying which call was refused sends the next reader to
/// guess.
fn run_two_writers(path: &Path) -> (usize, String) {
    let first = spawn(path, "writer-a");
    let second = spawn(path, "writer-b");
    let outputs: Vec<_> = [first, second]
        .into_iter()
        .map(|child| child.wait_with_output().expect("the writer exits"))
        .collect();

    let finished = outputs.iter().filter(|o| o.status.success()).count();
    let complaints = outputs
        .iter()
        .filter(|o| !o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (finished, complaints)
}

/// Two processes appending to their own event streams.
///
/// What a single-process test cannot say: that the global order the store
/// hands out survives two writers bumping one counter, and that neither
/// stream's chain is disturbed by the other's appends landing between its
/// own.
#[tokio::test]
async fn two_processes_append_to_one_event_store_and_nothing_is_lost() {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("ceremonies.sqlite3");

    let (finished, complaints) = run_two_writers(&path);
    assert_eq!(
        finished, 2,
        "both writers must finish on the SQLite event store.\n{complaints}"
    );

    let store = SqliteCeremonyStore::open(&path).expect("the store reopens");
    let streams = store.streams().await.expect("streams list");
    assert_eq!(
        streams,
        [
            CeremonyId::new("writer-a").unwrap(),
            CeremonyId::new("writer-b").unwrap()
        ]
    );

    for name in ["writer-a", "writer-b"] {
        let ceremony = CeremonyId::new(name).unwrap();

        let records = store
            .read(
                &ceremony,
                StreamVersion::EMPTY,
                CeremonyEventPageLimit::DEFAULT,
            )
            .await
            .expect("the stream reads");
        assert_eq!(
            records.len() as u64,
            EVENTS_PER_WRITER,
            "{name} lost events to the other writer"
        );
        for (index, record) in records.iter().enumerate() {
            assert_eq!(
                record.sequence().value(),
                index as u64 + 1,
                "{name} stream is out of order at {index}"
            );
        }
        assert!(
            AuditChain::verify(&records).is_intact(),
            "{name} stream does not chain after interleaved appends"
        );
        assert_eq!(
            CeremonyEventStorePort::head(&store, &ceremony)
                .await
                .expect("head reads"),
            StreamVersion::new(EVENTS_PER_WRITER)
        );
    }

    // One global order across both processes: every event of either
    // stream sits at its own position, and nothing was skipped or
    // handed out twice while the two counters raced.
    let rows = store
        .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
        .await
        .expect("the global order reads");
    assert_eq!(rows.len() as u64, 2 * EVENTS_PER_WRITER);
    let positions: Vec<u64> = rows.iter().map(|row| row.position.value()).collect();
    assert_eq!(
        positions,
        (1..=2 * EVENTS_PER_WRITER).collect::<Vec<_>>(),
        "global positions are not contiguous and strictly increasing"
    );
}
