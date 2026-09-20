//! The clustered agentic-system stores against the same contracts the
//! embedded ones answer.
//!
//! Three replicas sharing one database is where the compare-and-swap
//! actually earns its keep: in memory the map lock does the work, and
//! in SQLite one write transaction does, but on Postgres the head has
//! to be taken under a row lock or two replicas both write the same
//! next revision and one edit disappears.

#![cfg(feature = "container-tests")]

use made_adapters::postgres::{
    PostgresAgenticSystemExecutions, PostgresAgenticSystemPublications,
    PostgresAgenticSystemRepository,
};
use made_core::conformance::{
    AgenticSystemExecutionStoreConformance, AgenticSystemPublicationConformance,
    AgenticSystemRepositoryConformance,
};
use made_tests_integration::postgres_fixture;

#[tokio::test]
async fn postgres_satisfies_the_agentic_system_contracts() {
    let (pool, _url, _container) = postgres_fixture::start_with_url().await;

    let repository = PostgresAgenticSystemRepository::new(pool.clone());
    let passed = AgenticSystemRepositoryConformance::run(&repository)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(passed.len(), 5, "repository properties run: {passed:?}");

    let publications = PostgresAgenticSystemPublications::new(pool.clone());
    let passed = AgenticSystemPublicationConformance::run(&publications)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(passed.len(), 5, "publication properties run: {passed:?}");

    let executions = PostgresAgenticSystemExecutions::new(pool);
    let passed = AgenticSystemExecutionStoreConformance::run(&executions)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(passed.len(), 4, "execution properties run: {passed:?}");
}
