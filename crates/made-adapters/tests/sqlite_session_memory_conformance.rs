#![cfg(feature = "sqlite")]

//! The durable memory adapter against the port contract and a cold reopen.

use made_adapters::sqlite::SqliteSessionMemory;
use made_core::conformance::MemoryConformance;
use made_core::ports::{MemoryReaderPort, MemoryWriteOutcome, MemoryWriterPort};
use made_core::value_objects::{
    Attributes, CeremonyId, MemoryEntry, MemoryEntryId, MemoryEntryKind, MemoryProvenance,
    MemoryScope, MemoryWrite,
};
use time::OffsetDateTime;

#[tokio::test]
async fn sqlite_memory_satisfies_the_contract() {
    let directory = tempfile::tempdir().unwrap();
    let memory = SqliteSessionMemory::open(directory.path().join("memory.sqlite3")).unwrap();

    let passed = MemoryConformance::run(&memory, &memory)
        .await
        .expect("SQLite memory must satisfy every property it declares");

    assert_eq!(passed.len(), 9);
}

#[tokio::test]
async fn remembered_decision_survives_a_cold_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("memory.sqlite3");
    let scope = MemoryScope::new("team:restart").unwrap();
    let write = MemoryWrite::unexplained(vec![MemoryEntry::new(
        MemoryEntryId::new("restart:decision").unwrap(),
        MemoryEntryKind::Decision,
        "roll back rather than restart",
        MemoryProvenance::new(
            CeremonyId::new("first-process").unwrap(),
            None,
            OffsetDateTime::UNIX_EPOCH,
        ),
        Attributes::empty(),
    )
    .unwrap()])
    .unwrap();

    {
        let first = SqliteSessionMemory::open(&path).unwrap();
        assert_eq!(
            first
                .remember(&scope, write, "restart:write")
                .await
                .unwrap(),
            MemoryWriteOutcome::Remembered
        );
    }

    let reopened = SqliteSessionMemory::open(&path).unwrap();
    let recalled = reopened.recall(&scope).await.unwrap();
    assert_eq!(recalled.entries().len(), 1);
    assert_eq!(
        recalled.entries()[0].summary(),
        "roll back rather than restart"
    );
}
