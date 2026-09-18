#![cfg(feature = "container-tests")]
use made_adapters::council_data_snapshot::CouncilDataSnapshot;
use made_adapters::postgres::{PostgresCouncilJournal, PostgresCouncilSnapshot};
use made_adapters::sqlite::{SqliteCouncilJournal, SqliteCouncilSnapshot, SqliteCouncilStore};
use made_core::entities::{
    Council, CouncilJournalEvent, CouncilSnapshotProvenance, Deliberation, Statistics,
};
use made_core::ports::{AgentDescriptor, CouncilJournalPort};
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, CouncilJournalPageLimit, CouncilSnapshotSource,
    DurationMs, OutputContract, OutputFormat, Rounds, Specialty, TaskId,
};
use std::collections::BTreeMap;
use time::macros::datetime;

fn snapshot(source: &str) -> CouncilDataSnapshot {
    let now = datetime!(2026-09-18 00:00:00 UTC);
    let specialty = Specialty::new("research").unwrap();
    let agent = AgentDescriptor {
        id: AgentId::new("researcher").unwrap(),
        specialty: specialty.clone(),
        kind: AgentKind::new("vllm").unwrap(),
        attributes: Attributes::new(BTreeMap::from([(
            "provider.model".into(),
            serde_json::json!("deterministic-fixture-label"),
        )]))
        .unwrap(),
    };
    let mut statistics = Statistics::default();
    statistics.record_deliberation(&specialty, DurationMs::from_millis(19));
    CouncilDataSnapshot {
        schema_version: 1,
        provenance: CouncilSnapshotProvenance::new(
            CouncilSnapshotSource::new(source).unwrap(),
            now,
        ),
        councils: vec![Council::new(
            CouncilId::new("research").unwrap(),
            specialty.clone(),
            [agent.id.clone()],
            now,
        )
        .unwrap()],
        agents: vec![agent],
        contracts: vec![
            OutputContract::new("report", OutputFormat::JsonObject, BTreeMap::new()).unwrap(),
        ],
        deliberations: vec![Deliberation::start(
            TaskId::new("task-1").unwrap(),
            specialty,
            Rounds::default(),
            now,
        )],
        statistics,
    }
}

#[tokio::test]
async fn postgres_and_sqlite_import_one_snapshot_fact_without_fabricated_history() {
    let (pool, _container) = made_tests_integration::postgres_fixture::start().await;
    let pg = PostgresCouncilSnapshot::new(pool.clone());
    let original = snapshot("legacy-export-20260918");
    let receipt = pg
        .import(CouncilDataSnapshot::decode(&original.encode().unwrap()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        receipt.event(),
        &CouncilJournalEvent::SnapshotImported(original.provenance.clone())
    );
    assert_eq!(pg.import(original.clone()).await.unwrap(), receipt);
    let exported = pg.export(original.provenance.clone()).await.unwrap();
    assert_eq!(exported, original);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&path).unwrap();
    let directory = tempfile::tempdir_in(path).unwrap();
    let db = directory.path().join("import.sqlite3");
    let local = SqliteCouncilSnapshot::new(SqliteCouncilStore::open(&db).unwrap());
    assert_eq!(local.import(exported).await.unwrap(), receipt);
    drop(local);
    let reopened = SqliteCouncilStore::open(&db).unwrap();
    let local = SqliteCouncilSnapshot::new(reopened.clone());
    assert_eq!(
        local.export(original.provenance.clone()).await.unwrap(),
        original
    );
    assert_eq!(local.import(original.clone()).await.unwrap(), receipt);
    let expected = vec![receipt];
    assert_eq!(
        PostgresCouncilJournal::new(pool)
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        SqliteCouncilJournal::new(reopened)
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        expected
    );
}

#[tokio::test]
async fn conflicting_or_secret_snapshots_leave_data_and_provenance_unchanged() {
    let (pool, _container) = made_tests_integration::postgres_fixture::start().await;
    let pg = PostgresCouncilSnapshot::new(pool.clone());
    let original = snapshot("legacy-import");
    let expected = pg.import(original.clone()).await.unwrap();
    let mut changed = original.clone();
    changed
        .statistics
        .record_orchestration(DurationMs::from_millis(7));
    assert!(pg.import(changed.clone()).await.is_err());
    changed.provenance = snapshot("another-import").provenance;
    assert!(pg.import(changed).await.is_err());
    let mut secret = original.clone();
    secret.agents[0].attributes = Attributes::new(BTreeMap::from([(
        "provider.api_key".into(),
        serde_json::json!("fixture-not-a-secret"),
    )]))
    .unwrap();
    assert!(pg.import(secret.clone()).await.is_err());
    assert!(CouncilDataSnapshot::decode(&serde_json::to_vec(&secret).unwrap()).is_err());
    assert_eq!(
        pg.export(original.provenance.clone()).await.unwrap(),
        original
    );
    assert_eq!(
        PostgresCouncilJournal::new(pool)
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        vec![expected]
    );
}

#[test]
fn import_validation_rejects_invalid_legacy_deserialized_values() {
    let original = snapshot("validation-fixture");
    let mutations = [
        ("/councils/0/agents", serde_json::json!([])),
        ("/agents/0/id", serde_json::json!("")),
        ("/councils/0/specialty", serde_json::json!("  research  ")),
        ("/deliberations/0/rounds_budget", serde_json::json!(999)),
        (
            "/contracts/0/fields",
            serde_json::json!({"": {"required": true}}),
        ),
        (
            "/deliberations/0/ranking",
            serde_json::json!(["missing-proposal"]),
        ),
    ];
    for (pointer, bad) in mutations {
        let mut value = serde_json::to_value(&original).unwrap();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            CouncilDataSnapshot::decode(&serde_json::to_vec(&value).unwrap()).is_err(),
            "accepted invalid {pointer}"
        );
    }
}
