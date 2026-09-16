//! The in-memory event store against the stream and snapshot contracts.

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_core::conformance::{CeremonyEventStoreConformance, CeremonySnapshotStoreConformance};

#[tokio::test]
async fn the_in_memory_store_satisfies_the_event_store_contract() {
    let store = InMemoryCeremonyEventStore::new();

    let passed = CeremonyEventStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 13, "properties run: {passed:?}");
    assert!(passed.contains(&"a_stale_expectation_conflicts_and_writes_nothing"));
    assert!(passed.contains(&"concurrent_appends_admit_exactly_one_winner"));
}

#[tokio::test]
async fn the_in_memory_store_satisfies_the_snapshot_store_contract() {
    let store = InMemoryCeremonyEventStore::new();

    let passed = CeremonySnapshotStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}
