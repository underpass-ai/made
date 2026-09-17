use made_adapters::memory::InMemoryCeremonyEventCursor;
use made_core::conformance::CeremonyEventCursorConformance;

#[tokio::test]
async fn the_in_memory_cursor_satisfies_the_contract() {
    let cursor = InMemoryCeremonyEventCursor::new();

    let passed = CeremonyEventCursorConformance::run(&cursor)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 7, "properties run: {passed:?}");
    assert!(passed.contains(&"a_live_lease_excludes_another_worker"));
    assert!(passed.contains(&"failures_retry_and_quarantine_is_visible"));
}
