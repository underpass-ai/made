#![cfg(feature = "container-tests")]
use made_adapters::postgres::{
    PostgresContractRegistry, PostgresCouncilJournal, PostgresCouncilRegistry,
    PostgresDeliberationRepository, PostgresStatistics,
};
use made_adapters::sqlite::{
    SqliteContractRegistry, SqliteCouncilJournal, SqliteCouncilRegistry, SqliteCouncilStatistics,
    SqliteCouncilStore, SqliteDeliberationRepository,
};
use made_app::services::AuthorizationOperationScope;
use made_core::entities::{Council, CouncilJournalEvent, CouncilJournalRecord, Deliberation};
use made_core::events::{EventEnvelope, PhaseChangedEvent};
use made_core::ports::{
    ContractRegistryPort, CouncilJournalPort, CouncilRegistryPort, DeliberationRepositoryPort,
    StatisticsPort,
};
use made_core::value_objects::{
    AgentId, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationEvidence,
    AuthorizedOperation, CouncilId, CouncilJournalConsumer, CouncilJournalPageLimit,
    CouncilJournalPosition, DurationMs, EventId, OutputContract, OutputFormat, PrincipalId,
    PrincipalKind, Rounds, Specialty, TaskId,
};
use made_tests_integration::postgres_fixture;
use std::collections::BTreeMap;
use time::{macros::datetime, OffsetDateTime};

const NOW: OffsetDateTime = datetime!(2026-09-19 00:00:00 UTC);

fn authorized_operation() -> AuthorizedOperation {
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("council-operator").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::MutualTls,
    )
    .unwrap();
    let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
        "decision_id": "a".repeat(64),
        "request_id": "council-conformance-request",
        "principal_id": "council-operator",
        "action": "create_council",
        "scope": { "kind": "global" },
        "target_digest": "b".repeat(64),
        "policy_version": 1,
        "admitted_at": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T12:01:00Z"
    }))
    .unwrap();
    AuthorizedOperation::new(principal, evidence).unwrap()
}

fn publication(id: &str, phase: &str) -> CouncilJournalEvent {
    CouncilJournalEvent::PhaseChanged(
        PhaseChangedEvent::new(
            EventEnvelope::new(EventId::new(id).unwrap(), NOW, "conformance", None).unwrap(),
            TaskId::new("research-1").unwrap(),
            "proposing",
            phase,
        )
        .unwrap(),
    )
}

async fn exercise_registries(
    councils: &dyn CouncilRegistryPort,
    contracts: &dyn ContractRegistryPort,
    deliberations: &dyn DeliberationRepositoryPort,
    statistics: &dyn StatisticsPort,
    journal: &dyn CouncilJournalPort,
) -> Vec<CouncilJournalRecord> {
    let specialty = Specialty::new("research").unwrap();
    let council = Council::new(
        CouncilId::new("research-council").unwrap(),
        specialty.clone(),
        [AgentId::new("writer").unwrap()],
        NOW,
    )
    .unwrap();
    councils.register(council.clone()).await.unwrap();
    assert!(councils.register(council.clone()).await.is_err());
    assert_eq!(councils.get(&specialty).await.unwrap(), council);
    councils.replace(council).await.unwrap();
    let contract =
        OutputContract::new("report-v1", OutputFormat::JsonObject, BTreeMap::new()).unwrap();
    contracts.register(contract.clone()).await.unwrap();
    assert!(contracts.register(contract.clone()).await.is_err());
    assert_eq!(
        contracts.get(contract.contract_id()).await.unwrap(),
        contract
    );
    let deliberation = Deliberation::start(
        TaskId::new("research-1").unwrap(),
        specialty.clone(),
        Rounds::default(),
        NOW,
    );
    deliberations.save(&deliberation).await.unwrap();
    deliberations.save(&deliberation).await.unwrap();
    assert_eq!(
        deliberations.get(deliberation.task_id()).await.unwrap(),
        deliberation
    );
    statistics
        .record_deliberation(&specialty, DurationMs::from_millis(11))
        .await
        .unwrap();
    statistics
        .record_orchestration(DurationMs::from_millis(29))
        .await
        .unwrap();
    assert_eq!(
        statistics.snapshot().await.unwrap().total_duration().get(),
        40
    );
    let record = journal
        .publish(publication("event-1", "reviewing"))
        .await
        .unwrap();
    assert_eq!(
        journal
            .publish(publication("event-1", "reviewing"))
            .await
            .unwrap(),
        record
    );
    assert!(journal
        .publish(publication("event-1", "failed"))
        .await
        .is_err());
    councils.delete(&specialty).await.unwrap();
    assert!(councils.delete(&specialty).await.is_err());
    contracts.delete(contract.contract_id()).await.unwrap();
    assert!(contracts.delete(contract.contract_id()).await.is_err());
    assert!(councils.list().await.unwrap().is_empty());
    assert!(contracts.list().await.unwrap().is_empty());
    journal
        .read(None, CouncilJournalPageLimit::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn sqlite_and_postgres_commit_the_same_council_facts_and_preserve_old_repositories() {
    let (pool, _container) = postgres_fixture::start().await;
    let pg = PostgresCouncilJournal::new(pool.clone());
    let expected = AuthorizationOperationScope::run(
        authorized_operation(),
        exercise_registries(
            &PostgresCouncilRegistry::new(pool.clone()),
            &PostgresContractRegistry::new(pool.clone()),
            &PostgresDeliberationRepository::new(pool.clone()),
            &PostgresStatistics::new(pool.clone()),
            &pg,
        ),
    )
    .await;
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let local = SqliteCouncilStore::open(directory.path().join("compare.sqlite3")).unwrap();
    let actual = AuthorizationOperationScope::run(
        authorized_operation(),
        exercise_registries(
            &SqliteCouncilRegistry::new(local.clone()),
            &SqliteContractRegistry::new(local.clone()),
            &SqliteDeliberationRepository::new(local.clone()),
            &SqliteCouncilStatistics::new(local.clone()),
            &SqliteCouncilJournal::new(local),
        ),
    )
    .await;
    assert_eq!(actual, expected);
    assert_eq!(
        actual.len(),
        9,
        "failed commands and repeated publications do not append"
    );
    assert!(actual.iter().all(|record| record
        .authorization()
        .is_some_and(|evidence| evidence.principal_id().as_str() == "council-operator")));
    assert_eq!(
        PostgresCouncilJournal::new(pool)
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        expected
    );
}

#[tokio::test]
async fn postgres_concurrent_publication_and_cursor_lease_are_atomic() {
    let (pool, _container) = postgres_fixture::start().await;
    let first = PostgresCouncilJournal::new(pool.clone());
    let second = PostgresCouncilJournal::new(pool.clone());
    let (left, right) = tokio::join!(
        first.publish(publication("same-id", "reviewing")),
        second.publish(publication("same-id", "reviewing"))
    );
    assert_eq!(left.unwrap(), right.unwrap());
    let consumer = CouncilJournalConsumer::new("reader").unwrap();
    let (left, right) = tokio::join!(
        first.lease(&consumer, NOW, DurationMs::from_millis(1000)),
        second.lease(&consumer, NOW, DurationMs::from_millis(1000))
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert_eq!(
        usize::from(left.is_some()) + usize::from(right.is_some()),
        1
    );
    let retired = left.or(right).unwrap();
    let later = NOW + time::Duration::seconds(2);
    let current = second
        .lease(&consumer, later, DurationMs::from_millis(1000))
        .await
        .unwrap()
        .unwrap();
    assert!(first
        .acknowledge(&retired, CouncilJournalPosition::FIRST, later)
        .await
        .is_err());
    assert!(first.release(&retired, later).await.is_err());
    assert!(second
        .acknowledge(&current, CouncilJournalPosition::new(2).unwrap(), later)
        .await
        .is_err());
    second
        .acknowledge(&current, CouncilJournalPosition::FIRST, later)
        .await
        .unwrap();
    let reopened = PostgresCouncilJournal::new(pool);
    assert_eq!(
        reopened.position(&consumer).await.unwrap(),
        Some(CouncilJournalPosition::FIRST)
    );
    assert!(reopened
        .read(
            Some(CouncilJournalPosition::FIRST),
            CouncilJournalPageLimit::default()
        )
        .await
        .unwrap()
        .is_empty());
}
