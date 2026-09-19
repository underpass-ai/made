#![cfg(feature = "sqlite")]

use std::process::Command;

use made_adapters::sqlite::SqliteCeremonyStore;
use rusqlite::{Connection, ErrorCode};

const PROBE_PATH: &str = "MADE_TEST_SQLITE_LOCK_PROBE_PATH";

#[test]
fn opening_a_second_adapter_keeps_the_first_adapters_process_lock() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let path = directory.path().join("store.sqlite3");
    let first = SqliteCeremonyStore::open(&path).unwrap();
    let second = SqliteCeremonyStore::open(&path).unwrap();

    // Changing WAL mode needs an exclusive database lock. A different process
    // must fail while either adapter remains open, even when the second open
    // performs format detection on the same file.
    let probe = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "competing_process_cannot_replace_wal",
            "--ignored",
            "--nocapture",
        ])
        .env(PROBE_PATH, &path)
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "competing process escaped the live adapters' locks: {} {}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    );
    assert_eq!(first.legacy_instances_without_a_stream().unwrap(), 0);
    assert_eq!(second.legacy_instances_without_a_stream().unwrap(), 0);
}

#[test]
#[ignore = "spawned by opening_a_second_adapter_keeps_the_first_adapters_process_lock"]
fn competing_process_cannot_replace_wal() {
    let Some(path) = std::env::var_os(PROBE_PATH) else {
        return;
    };
    let connection = Connection::open(path).unwrap();
    connection
        .busy_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    let result = connection.pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0));
    assert_eq!(result.unwrap().to_lowercase(), "wal");
    let refusal = connection
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap_err();
    assert_eq!(refusal.sqlite_error_code(), Some(ErrorCode::DatabaseBusy));
}
