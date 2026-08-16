//! The reason the SQLite engine exists: two OS processes open one store and
//! both work.
//!
//! On redb this is the failure a MADE user actually hits — a Codex session
//! holding the store while Claude Code starts, or the reverse — and the test
//! says so on purpose. Pinning the difference between the engines with an
//! assertion rather than prose means the day redb grows multi-process
//! support, this fails loudly instead of quietly staying true.

#![cfg(feature = "sqlite")]

use std::path::Path;
use std::process::{Command, Stdio};

use made_adapters::redb::RedbCeremonyStore;
use made_core::ports::{AuditJournalPort, CeremonyInstanceRepositoryPort};
use made_core::value_objects::CeremonyId;
use tempfile::TempDir;

const COMMITS_PER_WRITER: u64 = 40;

fn spawn(path: &Path, engine: &str, ceremony: &str) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_store_writer"))
        .arg(path)
        .arg(engine)
        .arg(ceremony)
        .arg(COMMITS_PER_WRITER.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the writer spawns")
}

/// Runs two writers concurrently on one store and returns how many finished.
fn run_two_writers(path: &Path, engine: &str) -> usize {
    let first = spawn(path, engine, "writer-a");
    let second = spawn(path, engine, "writer-b");
    [first, second]
        .into_iter()
        .map(|child| child.wait_with_output().expect("the writer exits"))
        .filter(|output| output.status.success())
        .count()
}

#[tokio::test]
async fn two_processes_write_one_sqlite_store_and_nothing_is_lost() {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("ceremonies.sqlite3");

    let finished = run_two_writers(&path, "sqlite");
    assert_eq!(
        finished, 2,
        "both writers must finish on the sqlite engine; that is what it is for"
    );

    let store = RedbCeremonyStore::open_sqlite(&path).expect("the store reopens");
    for name in ["writer-a", "writer-b"] {
        let ceremony = CeremonyId::new(name).unwrap();

        // Each writer advanced its own ceremony by exactly its own commits:
        // interleaving two processes must not cost either of them a revision.
        let instance = store.get(&ceremony).await.expect("the ceremony is stored");
        assert_eq!(instance.id(), &ceremony);

        let records = store.records(&ceremony).await.expect("the journal reads");
        assert_eq!(
            records.len() as u64,
            COMMITS_PER_WRITER,
            "{name} lost journal records to the other writer"
        );

        // The journal is a hash chain; a shuffled or gapped scan breaks it.
        // This is what proves the seam's byte-order contract holds on SQLite,
        // where the ordinal is a big-endian suffix inside a BLOB key.
        for (index, record) in records.iter().enumerate() {
            assert_eq!(
                record.sequence().value(),
                index as u64 + 1,
                "{name} journal is out of order at {index}"
            );
        }
    }
}

#[tokio::test]
async fn on_redb_the_second_process_is_refused_and_the_first_loses_nothing() {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("ceremonies.redb");

    let finished = run_two_writers(&path, "redb");
    assert_eq!(
        finished, 1,
        "redb is single-process: exactly one writer holds the store"
    );

    // The one that got in must have lost nothing to the one that did not.
    let store = RedbCeremonyStore::open(&path).expect("the store reopens");
    let mut total = 0u64;
    for name in ["writer-a", "writer-b"] {
        let ceremony = CeremonyId::new(name).unwrap();
        total += store
            .records(&ceremony)
            .await
            .expect("the journal reads")
            .len() as u64;
    }
    assert_eq!(
        total, COMMITS_PER_WRITER,
        "the surviving writer's records are all there, and only those"
    );
}
