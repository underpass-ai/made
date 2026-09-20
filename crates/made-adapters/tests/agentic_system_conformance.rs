//! The agentic-system stores against their contracts, in memory.

use made_adapters::memory::{
    InMemoryAgenticSystemExecutions, InMemoryAgenticSystemPublications,
    InMemoryAgenticSystemRepository,
};
use made_core::conformance::{
    AgenticSystemExecutionStoreConformance, AgenticSystemPublicationConformance,
    AgenticSystemRepositoryConformance,
};

#[tokio::test]
async fn the_in_memory_repository_satisfies_the_contract() {
    let repository = InMemoryAgenticSystemRepository::new();

    let passed = AgenticSystemRepositoryConformance::run(&repository)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
    assert!(passed.contains(&"a_concurrent_edit_is_refused_rather_than_overwritten"));
    assert!(passed.contains(&"an_earlier_revision_stays_readable"));
}

#[tokio::test]
async fn the_in_memory_publications_satisfy_the_contract() {
    let publications = InMemoryAgenticSystemPublications::new();

    let passed = AgenticSystemPublicationConformance::run(&publications)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
    assert!(passed.contains(&"a_taken_revision_is_never_overwritten"));
}

#[tokio::test]
async fn the_in_memory_executions_satisfy_the_contract() {
    let executions = InMemoryAgenticSystemExecutions::new();

    let passed = AgenticSystemExecutionStoreConformance::run(&executions)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 4, "properties run: {passed:?}");
    assert!(passed.contains(&"opening_the_same_run_twice_returns_the_first"));
}
