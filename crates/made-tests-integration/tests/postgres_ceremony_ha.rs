//! Process-level failure and restart evidence for the shared ceremony store.

#![cfg(feature = "container-tests")]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use made_adapters::postgres::{PostgresCeremonyStore, PostgresConfig, PostgresPool};
use made_core::entities::ceremony_events::CeremonyCompleted;
use made_core::entities::{AuditFact, CeremonyEvent};
use made_core::ports::{
    AppendOutcome, CeremonyEventCursorPort, CeremonyEventStorePort, ExecutionReceiptStorePort,
};
use made_core::value_objects::{
    ArtifactSourceKind, AuditActor, AuditActorKind, CeremonyEventConsumer, CeremonyEventPageLimit,
    CeremonyId, CeremonyName, CeremonyVersion, EventId, ExecutionConnectorId, ExecutionIntent,
    ExecutionOperation, ExecutionReceipt, ExecutionRecoveryCapability, ExecutionRequestBytes,
    GlobalPosition, StateId, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
    StepOutput, StepResult, StreamVersion,
};
use made_core::DomainError;
use made_tests_integration::postgres_fixture;
use sqlx::postgres::PgPoolOptions;
use testcontainers::core::{CmdWaitFor, ExecCommand};
use time::OffsetDateTime;
use tokio::process::{Child, Command};

#[path = "postgres_ceremony_ha/claims.rs"]
mod claims;

const MODE: &str = "MADE_HA_HELPER_MODE";
const URL: &str = "MADE_HA_POSTGRES_URL";
const RESULT: &str = "MADE_HA_RESULT_PATH";
const SUFFIX: &str = "MADE_HA_SUFFIX";
const BLOCK_KEY: i64 = 19_091_905;

#[tokio::test]
async fn three_process_replicas_fence_append_and_claim() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let scratch = scratch();
    let mut append = [
        spawn_helper(&url, "append", "1", scratch.path()),
        spawn_helper(&url, "append", "2", scratch.path()),
        spawn_helper(&url, "append", "3", scratch.path()),
    ];
    wait_success(&mut append).await;
    let append_results = read_results(scratch.path(), "append", 3);
    assert_eq!(count(&append_results, "appended"), 1, "{append_results:?}");
    assert_eq!(count(&append_results, "conflict"), 2, "{append_results:?}");

    let store = Arc::new(PostgresCeremonyStore::new(pool));
    claims::start(store.clone(), "ha-process-claim").await;
    let mut claim = [
        spawn_helper(&url, "claim", "1", scratch.path()),
        spawn_helper(&url, "claim", "2", scratch.path()),
        spawn_helper(&url, "claim", "3", scratch.path()),
    ];
    wait_success(&mut claim).await;
    let claim_results = read_results(scratch.path(), "claim", 3);
    assert_eq!(count(&claim_results, "claimed"), 1, "{claim_results:?}");
    assert_eq!(count(&claim_results, "refused"), 2, "{claim_results:?}");
    let records = store
        .read(
            &CeremonyId::new("ha-process-claim").unwrap(),
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::DEFAULT,
        )
        .await
        .unwrap();
    assert_eq!(records.len(), 2, "opening plus exactly one real claim");
    assert!(matches!(
        records[1].event(),
        Some(CeremonyEvent::StepStarted(_))
    ));
}

#[tokio::test]
async fn killed_processes_leave_no_partial_append_or_claim() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let store = Arc::new(PostgresCeremonyStore::new(pool.clone()));
    kill_inside_event_insert(&url, "hold_append", "ha-killed-append", None).await;
    let stream = CeremonyId::new("ha-killed-append").unwrap();
    assert_eq!(store.head(&stream).await.unwrap(), StreamVersion::EMPTY);
    assert!(matches!(
        store
            .append(
                &stream,
                StreamVersion::EMPTY,
                vec![fact(&stream, "after-kill")]
            )
            .await
            .unwrap(),
        AppendOutcome::Appended { .. }
    ));

    claims::start(store.clone(), "ha-killed-claim").await;
    let id = CeremonyId::new("ha-killed-claim").unwrap();
    let before = store
        .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    kill_inside_event_insert(&url, "hold_claim", "ha-killed-claim", None).await;
    assert_eq!(store.head(&id).await.unwrap(), StreamVersion::new(1));
    assert_eq!(
        store
            .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
            .await
            .unwrap(),
        before
    );
    let accepted = claims::claim(store.clone(), "ha-killed-claim", "survivor")
        .await
        .unwrap();
    assert_eq!(accepted.version(), StreamVersion::new(2));
    // The global counter and journal also rolled back; no position was lost.
    let records = store
        .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(
        records
            .iter()
            .map(|record| record.position.value())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[tokio::test]
async fn two_live_replicas_recover_the_claim_when_the_third_dies_mid_append() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let store = Arc::new(PostgresCeremonyStore::new(pool));
    claims::start(store.clone(), "ha-killed-claim").await;
    let scratch = scratch();
    kill_inside_event_insert(&url, "hold_claim", "ha-killed-claim", Some(scratch.path())).await;
    let outcomes = read_results(scratch.path(), "claim_killed", 2);
    assert_eq!(count(&outcomes, "claimed"), 1, "{outcomes:?}");
    assert_eq!(count(&outcomes, "refused"), 1, "{outcomes:?}");
    let records = store
        .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].position.value(), 2);
    let Some(CeremonyEvent::StepStarted(claim)) = records[1].record.event() else {
        panic!("the survivor must seal a real claim");
    };
    assert_ne!(claim.lease.owner_id().as_str(), "replica-lost");
}

#[tokio::test]
async fn rolling_process_restarts_replay_the_journal_without_a_nats_notice() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let store = PostgresCeremonyStore::new(pool);
    let stream = CeremonyId::new("ha-replay").unwrap();
    let consumer = CeremonyEventConsumer::new("ha-replay-consumer").unwrap();
    let scratch = scratch();
    let mut version = StreamVersion::EMPTY;
    for round in 1..=3 {
        let outcome = store
            .append(
                &stream,
                version,
                vec![fact(&stream, &format!("round-{round}"))],
            )
            .await
            .unwrap();
        let AppendOutcome::Appended { version: next, .. } = outcome else {
            panic!("one writer owns the rolling stream");
        };
        version = next;
        let mut helper = spawn_helper(&url, "replay", &round.to_string(), scratch.path());
        assert!(helper.wait().await.unwrap().success());
        assert_eq!(
            std::fs::read_to_string(result_path(scratch.path(), "replay", &round.to_string()))
                .unwrap(),
            "1"
        );
        assert_eq!(
            store.position(&consumer).await.unwrap(),
            Some(GlobalPosition::new(round).unwrap())
        );
    }
}

#[tokio::test]
async fn pg_dump_restore_preserves_journal_hashes_and_receipts() {
    let (pool, url, container) = postgres_fixture::start_with_url().await;
    let store = PostgresCeremonyStore::new(pool);
    let stream = CeremonyId::new("ha-export").unwrap();
    store
        .append(
            &stream,
            StreamVersion::EMPTY,
            vec![fact(&stream, "exported")],
        )
        .await
        .unwrap();
    let operation = operation("ha-export");
    let producer = fence('7');
    store
        .record_intent(intent(operation.clone(), producer.clone()))
        .await
        .unwrap();
    let receipt = receipt(&operation, producer);
    store.record_receipt(receipt.clone()).await.unwrap();
    let original = store
        .read(
            &stream,
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::DEFAULT,
        )
        .await
        .unwrap();

    exec_ok(
        &container,
        [
            "pg_dump",
            "-U",
            "made",
            "-Fc",
            "-f",
            "/tmp/made.dump",
            "made",
        ],
    )
    .await;
    exec_ok(&container, ["createdb", "-U", "made", "made_restore"]).await;
    exec_ok(
        &container,
        [
            "pg_restore",
            "-U",
            "made",
            "-d",
            "made_restore",
            "/tmp/made.dump",
        ],
    )
    .await;
    let digest = exec_stdout(&container, ["sha256sum", "/tmp/made.dump"]).await;
    assert_eq!(digest.split_whitespace().next().unwrap().len(), 64);

    let restore_url = format!(
        "{}/made_restore",
        url.rsplit_once('/').expect("database URL has a path").0
    );
    let restored_pool = PostgresPool::connect(&PostgresConfig::from_url(restore_url))
        .await
        .unwrap();
    let restored = PostgresCeremonyStore::new(restored_pool);
    assert_eq!(
        restored
            .read(
                &stream,
                StreamVersion::EMPTY,
                CeremonyEventPageLimit::DEFAULT
            )
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        restored.receipt(operation.operation_id()).await.unwrap(),
        Some(receipt)
    );
}

#[tokio::test]
#[ignore = "subprocess entrypoint"]
async fn replica_process() {
    let mode = std::env::var(MODE).expect("helper mode");
    let url = std::env::var(URL).expect("helper url");
    match mode.as_str() {
        "append" => helper_append(&url).await,
        "claim" => helper_claim(&url, "ha-process-claim").await,
        "claim_killed" => helper_claim(&url, "ha-killed-claim").await,
        "replay" => helper_replay(&url).await,
        "hold_append" => hold_append(&url).await,
        "hold_claim" => hold_claim(&url).await,
        other => panic!("unknown helper mode {other}"),
    }
}

async fn helper_append(url: &str) {
    let store = connect(url).await;
    let suffix = std::env::var(SUFFIX).unwrap();
    let stream = CeremonyId::new("ha-process-append").unwrap();
    let outcome = store
        .append(&stream, StreamVersion::EMPTY, vec![fact(&stream, &suffix)])
        .await
        .unwrap();
    write_result(if outcome.is_conflict() {
        "conflict"
    } else {
        "appended"
    })
    .await;
}

async fn helper_claim(url: &str, ceremony: &str) {
    let store = Arc::new(connect(url).await);
    let suffix = std::env::var(SUFFIX).unwrap();
    let label = match claims::claim(store, ceremony, &suffix).await {
        Ok(_) => "claimed",
        Err(DomainError::InvariantViolated {
            reason: "step lease is still active",
        }) => "refused",
        unexpected => panic!("unexpected claim result: {unexpected:?}"),
    };
    write_result(label).await;
}

async fn helper_replay(url: &str) {
    let store = connect(url).await;
    let consumer = CeremonyEventConsumer::new("ha-replay-consumer").unwrap();
    let from = store
        .position(&consumer)
        .await
        .unwrap()
        .map_or(GlobalPosition::FIRST, GlobalPosition::next);
    let page = store
        .read_all(from, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    if let Some(last) = page.last() {
        store.acknowledge(&consumer, last.position).await.unwrap();
    }
    write_result(&page.len().to_string()).await;
}

async fn hold_append(url: &str) {
    let store = connect(url).await;
    let stream = CeremonyId::new("ha-killed-append").unwrap();
    store
        .append(&stream, StreamVersion::EMPTY, vec![fact(&stream, "killed")])
        .await
        .unwrap();
    panic!("test must kill this process inside the real adapter transaction");
}

async fn hold_claim(url: &str) {
    claims::claim(Arc::new(connect(url).await), "ha-killed-claim", "lost")
        .await
        .unwrap();
    panic!("test must kill this process inside the real claim append");
}

async fn kill_inside_event_insert(url: &str, mode: &str, stream: &str, survivors: Option<&Path>) {
    let pool = PgPoolOptions::new().connect(url).await.unwrap();
    // The isolated database's trigger blocks after the production adapter has
    // inserted a sealed event, before its global head/stream head and commit.
    sqlx::raw_sql(&format!(
        "CREATE OR REPLACE FUNCTION ha_block_insert() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.stream_id = '{stream}' THEN PERFORM pg_advisory_xact_lock({BLOCK_KEY}); \
         END IF; RETURN NEW; END $$; \
         CREATE TRIGGER ha_block_insert AFTER INSERT ON ceremony_events \
         FOR EACH ROW EXECUTE FUNCTION ha_block_insert();"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(BLOCK_KEY)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut child = helper_command(url, mode).spawn().unwrap();
    let pid = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let pid: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_locks WHERE locktype = 'advisory' AND objid = $1::bigint::oid AND NOT granted"
            ).bind(BLOCK_KEY).fetch_optional(&pool).await.unwrap();
            if let Some(pid) = pid { break pid; }
            assert!(child.try_wait().unwrap().is_none(), "helper exited before its adapter blocked");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("production insert should wait on the controlled lock");
    let mut live = survivors.map(|scratch| {
        [
            spawn_helper(url, "claim_killed", "1", scratch),
            spawn_helper(url, "claim_killed", "2", scratch),
        ]
    });
    if live.is_some() {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let waiting: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type = 'Lock' \
                     AND query LIKE '%ceremony_streams%'",
                )
                .fetch_one(&pool)
                .await
                .unwrap();
                if waiting == 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("both live replicas must contend with the uncommitted real claim");
    }
    child.kill().await.unwrap();
    assert!(!child.wait().await.unwrap().success());
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(BLOCK_KEY)
        .execute(&mut *blocker)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid = $1)")
                    .bind(pid)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if !exists {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server must retire killed client's uncommitted transaction");
    if let Some(children) = &mut live {
        for survivor in children {
            assert!(
                tokio::time::timeout(Duration::from_secs(15), survivor.wait())
                    .await
                    .unwrap()
                    .unwrap()
                    .success()
            );
        }
    }
    sqlx::query("DROP TRIGGER ha_block_insert ON ceremony_events")
        .execute(&pool)
        .await
        .unwrap();
}

async fn connect(url: &str) -> PostgresCeremonyStore {
    let mut config = PostgresConfig::from_url(url);
    config.acquire_timeout = Duration::from_secs(10);
    PostgresCeremonyStore::new(PostgresPool::connect(&config).await.unwrap())
}

async fn write_result(value: &str) {
    tokio::fs::write(std::env::var(RESULT).unwrap(), value)
        .await
        .unwrap();
}

fn spawn_helper(url: &str, mode: &str, suffix: &str, scratch: &Path) -> Child {
    let result = result_path(scratch, mode, suffix);
    helper_command(url, mode)
        .env(RESULT, result)
        .env(SUFFIX, suffix)
        .spawn()
        .unwrap()
}

fn helper_command(url: &str, mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--ignored", "--exact", "replica_process", "--nocapture"])
        .env(MODE, mode)
        .env(URL, url)
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

async fn wait_success(children: &mut [Child; 3]) {
    for child in children {
        assert!(child.wait().await.unwrap().success());
    }
}

fn scratch() -> tempfile::TempDir {
    std::fs::create_dir_all("tmp").unwrap();
    tempfile::tempdir_in("tmp").unwrap()
}

fn result_path(scratch: &Path, mode: &str, suffix: &str) -> PathBuf {
    scratch.join(format!("{mode}-{suffix}.result"))
}

fn read_results(scratch: &Path, mode: &str, total: usize) -> Vec<String> {
    (1..=total)
        .map(|index| {
            std::fs::read_to_string(result_path(scratch, mode, &index.to_string())).unwrap()
        })
        .collect()
}

fn count(values: &[String], wanted: &str) -> usize {
    values
        .iter()
        .filter(|value| value.as_str() == wanted)
        .count()
}

async fn exec_ok<const N: usize>(
    container: &testcontainers::ContainerAsync<testcontainers::GenericImage>,
    command: [&str; N],
) {
    let result = container
        .exec(ExecCommand::new(command).with_cmd_ready_condition(CmdWaitFor::exit_code(0)))
        .await
        .unwrap();
    assert_eq!(result.exit_code().await.unwrap(), Some(0));
}

async fn exec_stdout<const N: usize>(
    container: &testcontainers::ContainerAsync<testcontainers::GenericImage>,
    command: [&str; N],
) -> String {
    let mut result = container
        .exec(ExecCommand::new(command).with_cmd_ready_condition(CmdWaitFor::exit_code(0)))
        .await
        .unwrap();
    String::from_utf8(result.stdout_to_vec().await.unwrap()).unwrap()
}

fn fact(stream: &CeremonyId, event: &str) -> AuditFact {
    AuditFact {
        event_id: EventId::new(format!("ha-{event}")).unwrap(),
        event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        ceremony_id: stream.clone(),
        definition_name: CeremonyName::new("ha").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("ha-test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

fn operation(ceremony: &str) -> ExecutionOperation {
    ExecutionOperation::new(
        CeremonyId::new(ceremony).unwrap(),
        StepId::new("work").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(b"ha request".to_vec()).unwrap(),
    )
}

fn fence(digit: char) -> StepClaimFence {
    StepClaimFence::new(digit.to_string().repeat(64)).unwrap()
}

fn intent(operation: ExecutionOperation, claim_fence: StepClaimFence) -> ExecutionIntent {
    ExecutionIntent::new(
        operation,
        claim_fence,
        ExecutionConnectorId::new("ha.noop").unwrap(),
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::NoOp,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn receipt(operation: &ExecutionOperation, claim_fence: StepClaimFence) -> ExecutionReceipt {
    ExecutionReceipt::new(
        operation.operation_id().clone(),
        operation.request_digest().clone(),
        claim_fence,
        ExecutionConnectorId::new("ha.noop").unwrap(),
        None,
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::NoOp,
        StepResult::completed(StepOutput::empty()).unwrap(),
        Vec::new(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}
