#![cfg(feature = "sqlite")]

use async_trait::async_trait;
use made_adapters::noop::NoopAgent;
use made_adapters::sqlite::{
    SqliteAgentRegistry, SqliteContractRegistry, SqliteCouncilJournal, SqliteCouncilRegistry,
    SqliteCouncilStatistics, SqliteCouncilStore, SqliteDeliberationRepository,
};
use made_core::entities::{Council, CouncilJournalEvent, Deliberation};
use made_core::error::DomainError;
use made_core::events::{EventEnvelope, PhaseChangedEvent};
use made_core::ports::{
    AgentDescriptor, AgentFactoryPort, AgentPort, AgentRegistryPort, AgentResolverPort,
    ContractRegistryPort, CouncilJournalPort, CouncilRegistryPort, DeliberationRepositoryPort,
    StatisticsPort,
};
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, CouncilJournalConsumer, CouncilJournalPageLimit,
    CouncilJournalPosition, DurationMs, EventId, OutputContract, OutputFormat, Rounds, Specialty,
    TaskId,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use time::{macros::datetime, OffsetDateTime};

const NOW: OffsetDateTime = datetime!(2026-09-19 00:00:00 UTC);

fn scratch() -> tempfile::TempDir {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&path).unwrap();
    tempfile::tempdir_in(path).unwrap()
}
fn specialty() -> Specialty {
    Specialty::new("research").unwrap()
}
fn descriptor() -> AgentDescriptor {
    AgentDescriptor {
        id: AgentId::new("local-model").unwrap(),
        specialty: specialty(),
        kind: AgentKind::new("vllm").unwrap(),
        attributes: Attributes::new(BTreeMap::from([
            ("provider.model".into(), serde_json::json!("test-model")),
            (
                "provider.endpoint".into(),
                serde_json::json!("http://localhost:8080/v1"),
            ),
        ]))
        .unwrap(),
    }
}

#[derive(Debug, Default)]
struct RecordingFactory {
    seen: Mutex<Vec<AgentDescriptor>>,
}
#[async_trait]
impl AgentFactoryPort for RecordingFactory {
    async fn create(&self, descriptor: AgentDescriptor) -> Result<Arc<dyn AgentPort>, DomainError> {
        self.seen.lock().unwrap().push(descriptor.clone());
        Ok(Arc::new(NoopAgent::new(
            descriptor.id,
            descriptor.specialty,
        )))
    }
}
fn event(id: &str, to: &str) -> CouncilJournalEvent {
    CouncilJournalEvent::PhaseChanged(
        PhaseChangedEvent::new(
            EventEnvelope::new(EventId::new(id).unwrap(), NOW, "durability-test", None).unwrap(),
            TaskId::new("task-1").unwrap(),
            "proposing",
            to,
        )
        .unwrap(),
    )
}

async fn seed_statistics(store: SqliteCouncilStore) {
    let statistics = SqliteCouncilStatistics::new(store);
    statistics
        .record_deliberation(&specialty(), DurationMs::from_millis(17))
        .await
        .unwrap();
    statistics
        .record_orchestration(DurationMs::from_millis(23))
        .await
        .unwrap();
}

#[tokio::test]
async fn all_local_repositories_reopen_and_agent_kind_is_preserved() {
    let scratch = scratch();
    let path = scratch.path().join("councils.sqlite3");
    let factory = Arc::new(RecordingFactory::default());
    let expected = descriptor();
    let council = Council::new(
        CouncilId::new("research-council").unwrap(),
        specialty(),
        [expected.id.clone()],
        NOW,
    )
    .unwrap();
    let contract =
        OutputContract::new("report-v1", OutputFormat::JsonObject, BTreeMap::new()).unwrap();
    let deliberation = Deliberation::start(
        TaskId::new("task-1").unwrap(),
        specialty(),
        Rounds::default(),
        NOW,
    );
    {
        let store = SqliteCouncilStore::open(&path).unwrap();
        SqliteAgentRegistry::new(store.clone(), factory.clone())
            .register_described(
                expected.clone(),
                factory.create(expected.clone()).await.unwrap(),
            )
            .await
            .unwrap();
        SqliteCouncilRegistry::new(store.clone())
            .register(council.clone())
            .await
            .unwrap();
        SqliteContractRegistry::new(store.clone())
            .register(contract.clone())
            .await
            .unwrap();
        let repository = SqliteDeliberationRepository::new(store.clone());
        repository.save(&deliberation).await.unwrap();
        repository.save(&deliberation).await.unwrap();
        seed_statistics(store).await;
    }
    let store = SqliteCouncilStore::open(&path).unwrap();
    let agents = SqliteAgentRegistry::new(store.clone(), factory.clone());
    assert_eq!(agents.descriptor(&expected.id).await.unwrap(), expected);
    assert_eq!(
        agents.resolve(&expected.id).await.unwrap().id(),
        &expected.id
    );
    assert_eq!(factory.seen.lock().unwrap().last(), Some(&expected));
    assert_eq!(
        SqliteCouncilRegistry::new(store.clone())
            .get(&specialty())
            .await
            .unwrap(),
        council
    );
    assert_eq!(
        SqliteContractRegistry::new(store.clone())
            .get(contract.contract_id())
            .await
            .unwrap(),
        contract
    );
    assert_eq!(
        SqliteDeliberationRepository::new(store.clone())
            .get(deliberation.task_id())
            .await
            .unwrap(),
        deliberation
    );
    let stats = SqliteCouncilStatistics::new(store.clone())
        .snapshot()
        .await
        .unwrap();
    assert_eq!(
        (
            stats.total_deliberations(),
            stats.total_orchestrations(),
            stats.total_duration().get()
        ),
        (1, 1, 40)
    );
    let records = SqliteCouncilJournal::new(store)
        .read(None, CouncilJournalPageLimit::default())
        .await
        .unwrap();
    assert_eq!(
        records.len(),
        6,
        "re-saving the same snapshot adds no false history"
    );
    assert!(matches!(
        records[3].event(),
        CouncilJournalEvent::DeliberationSnapshotSaved(_)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_publication_is_idempotent_and_cursor_takeover_fences_the_old_worker() {
    let scratch = scratch();
    let path = scratch.path().join("journal.sqlite3");
    let first = SqliteCouncilJournal::new(SqliteCouncilStore::open(&path).unwrap());
    let second = SqliteCouncilJournal::new(SqliteCouncilStore::open(&path).unwrap());
    let fact = event("stable-event", "reviewing");
    let (left, right) = tokio::join!(first.publish(fact.clone()), second.publish(fact.clone()));
    assert_eq!(left.unwrap(), right.unwrap());
    assert!(second
        .publish(event("stable-event", "completed"))
        .await
        .is_err());
    let consumer = CouncilJournalConsumer::new("reporter").unwrap();
    let (left, right) = tokio::join!(
        first.lease(&consumer, NOW, DurationMs::from_millis(1000)),
        second.lease(&consumer, NOW, DurationMs::from_millis(1000))
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert_eq!(
        usize::from(left.is_some()) + usize::from(right.is_some()),
        1
    );
    let expired = left.or(right).unwrap();
    let later = NOW + time::Duration::seconds(2);
    let replacement = second
        .lease(&consumer, later, DurationMs::from_millis(1000))
        .await
        .unwrap()
        .unwrap();
    assert!(first
        .acknowledge(&expired, CouncilJournalPosition::FIRST, later)
        .await
        .is_err());
    assert!(first.release(&expired, later).await.is_err());
    assert!(second
        .acknowledge(&replacement, CouncilJournalPosition::new(2).unwrap(), later)
        .await
        .is_err());
    second
        .acknowledge(&replacement, CouncilJournalPosition::FIRST, later)
        .await
        .unwrap();
    drop(first);
    drop(second);
    let reopened = SqliteCouncilJournal::new(SqliteCouncilStore::open(&path).unwrap());
    assert_eq!(
        reopened.position(&consumer).await.unwrap(),
        Some(CouncilJournalPosition::FIRST)
    );
    assert_eq!(
        reopened
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap()
            .len(),
        1
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

#[tokio::test]
async fn refused_descriptors_and_registry_conflicts_leave_the_journal_unchanged() {
    let scratch = scratch();
    let store = SqliteCouncilStore::open(scratch.path().join("refused.sqlite3")).unwrap();
    let factory = Arc::new(RecordingFactory::default());
    let agents = SqliteAgentRegistry::new(store.clone(), factory.clone());
    for (key, value) in [
        ("provider.api_key", "not-a-real-secret"),
        ("provider.endpoint", "http://user:password@localhost/v1"),
        ("provider.endpoint", "http://localhost/v1?token=not-real"),
    ] {
        let mut bad = descriptor();
        bad.attributes =
            Attributes::new(BTreeMap::from([(key.to_owned(), serde_json::json!(value))])).unwrap();
        assert!(agents
            .register_described(bad.clone(), factory.create(bad).await.unwrap())
            .await
            .is_err());
    }
    let journal = SqliteCouncilJournal::new(store.clone());
    assert!(journal
        .read(None, CouncilJournalPageLimit::default())
        .await
        .unwrap()
        .is_empty());
    let good = descriptor();
    let handle = factory.create(good.clone()).await.unwrap();
    assert!(agents.register(handle.clone()).await.is_err());
    agents
        .register_described(good.clone(), handle.clone())
        .await
        .unwrap();
    assert!(agents.register_described(good, handle).await.is_err());
    assert_eq!(
        journal
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(serde_json::from_str::<CouncilJournalPosition>("0").is_err());
    assert!(serde_json::from_str::<CouncilJournalPageLimit>("0").is_err());
    assert!(serde_json::from_str::<CouncilJournalConsumer>("\" \"").is_err());
}

#[test]
fn council_process_child() {
    let Ok(path) = std::env::var("MADE_COUNCIL_CHILD_DB") else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = SqliteCouncilStore::open(path).unwrap();
        let journal = SqliteCouncilJournal::new(store.clone());
        journal
            .publish(event("cross-process-event", "reviewing"))
            .await
            .unwrap();
        SqliteCouncilStatistics::new(store)
            .record_orchestration(DurationMs::from_millis(7))
            .await
            .unwrap();
    });
}

#[tokio::test]
async fn independent_processes_share_journal_identity_and_durable_counters() {
    let scratch = scratch();
    let path = scratch.path().join("process.sqlite3");
    let binary = std::env::current_exe().unwrap();
    let mut children = (0..2)
        .map(|_| {
            std::process::Command::new(&binary)
                .args(["--exact", "council_process_child", "--nocapture"])
                .env("MADE_COUNCIL_CHILD_DB", &path)
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let store = SqliteCouncilStore::open(path).unwrap();
    assert_eq!(
        SqliteCouncilStatistics::new(store.clone())
            .snapshot()
            .await
            .unwrap()
            .total_orchestrations(),
        2
    );
    let records = SqliteCouncilJournal::new(store)
        .read(None, CouncilJournalPageLimit::default())
        .await
        .unwrap();
    assert_eq!(
        records.len(),
        3,
        "one publication, two real statistics observations"
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.event().publication_id().is_some())
            .count(),
        1
    );
}
