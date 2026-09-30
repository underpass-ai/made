#![cfg(feature = "container-postgres")]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use made_adapters::artifacts::ArtifactGcExclusionReason;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresBackupManifest, PostgresBackupService, PostgresCeremonyStore,
    PostgresConfig, PostgresPool,
};
use made_app::artifacts::ArtifactService;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactSnapshot, ArtifactStoreError,
    ArtifactStorePort, BeginArtifactUpload, ExecutionReceiptStorePort, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactRef,
    ArtifactSizeBytes, ArtifactSourceKind, AuditActorKind, CeremonyId, DurationMs,
    ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionReceipt,
    ExecutionReceiptId, ExecutionRecoveryCapability, ExecutionRequestBytes, IdempotencyKey,
    LeaseOwnerId, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration, StepLease,
    StepOutput, StepResult,
};
use made_tests_integration::postgres_fixture::start_with_url;
use sha2::{Digest, Sha256};
use testcontainers::core::{CmdWaitFor, ExecCommand};
use time::OffsetDateTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

/// Scale a wall-clock budget by MADE_TEST_TIMING_SCALE. The coverage job
/// instruments every test binary, which multiplies runner latency; the
/// scaling constant lets the same budgets hold there instead of failing on
/// instrumentation speed. Defaults to 1 for a plain cargo test run.
fn scaled_budget(budget: std::time::Duration) -> std::time::Duration {
    let scale: f64 = std::env::var("MADE_TEST_TIMING_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|scale| *scale >= 1.0)
        .unwrap_or(1.0);
    std::time::Duration::from_secs_f64(budget.as_secs_f64() * scale)
}

fn operation_fixture(ceremony: &str) -> (ExecutionOperation, ExecutionIntent, StepClaimFence) {
    let operation = ExecutionOperation::new(
        CeremonyId::new(ceremony).unwrap(),
        StepId::new("write_artifact").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(b"postgres receipt request".to_vec()).unwrap(),
    );
    let claim_fence = StepClaimFence::new("4".repeat(64)).unwrap();
    let intent = ExecutionIntent::new(
        operation.clone(),
        claim_fence.clone(),
        ExecutionConnectorId::new("postgres.noop").unwrap(),
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::NoOp,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    (operation, intent, claim_fence)
}

fn receipt_fixture(
    operation: &ExecutionOperation,
    claim_fence: StepClaimFence,
    artifact: ArtifactRef,
) -> ExecutionReceipt {
    ExecutionReceipt::new(
        operation.operation_id().clone(),
        operation.request_digest().clone(),
        claim_fence,
        ExecutionConnectorId::new("postgres.noop").unwrap(),
        None,
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::NoOp,
        StepResult::completed(StepOutput::empty()).unwrap(),
        vec![artifact],
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

async fn upload(
    store: &PostgresArtifactStore,
    bytes: &[u8],
) -> made_core::value_objects::ArtifactRef {
    let upload = store
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: digest(bytes),
            size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
            media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
            provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
            idempotency_key: ArtifactIdempotencyKey::new(format!(
                "postgres-protection-upload-{:x}",
                Sha256::digest(bytes)
            ))
            .unwrap(),
        })
        .await
        .unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes.to_vec(),
            chunk_digest: digest(bytes),
        })
        .await
        .unwrap();
    store.commit_upload(&upload.upload_id).await.unwrap()
}

async fn upload_receipt_artifact(
    store: &PostgresArtifactStore,
    bytes: &[u8],
    operation: &ExecutionOperation,
    claim_fence: &StepClaimFence,
) -> ArtifactRef {
    let artifact_digest = digest(bytes);
    let upload = store
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: artifact_digest.clone(),
            size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
            media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
            provenance: ArtifactProvenance::execution(
                ArtifactSourceKind::NoOp,
                ExecutionReceiptId::for_operation(operation.operation_id()),
                operation.operation_id().clone(),
                claim_fence.clone(),
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
            idempotency_key: ArtifactIdempotencyKey::new(format!(
                "postgres-receipt-upload-{:x}",
                Sha256::digest(bytes)
            ))
            .unwrap(),
        })
        .await
        .unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes.to_vec(),
            chunk_digest: artifact_digest,
        })
        .await
        .unwrap();
    store.commit_upload(&upload.upload_id).await.unwrap()
}

type PostgresContainer = testcontainers::ContainerAsync<testcontainers::GenericImage>;

const MAINTENANCE_LEASE: DurationMs = DurationMs::from_millis(300_000);

fn maintenance_lease(owner: &str, key: &str, now: OffsetDateTime) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new(owner).unwrap(),
        IdempotencyKey::new(key).unwrap(),
        now,
        MAINTENANCE_LEASE,
    )
    .unwrap()
}

async fn reconnect(url: &str) -> PostgresArtifactStore {
    PostgresArtifactStore::new(
        PostgresPool::connect(&PostgresConfig::from_url(url.to_owned()))
            .await
            .unwrap(),
    )
}

fn sibling_database_url(url: &str, database: &str) -> String {
    format!(
        "{}/{database}",
        url.rsplit_once('/').expect("database URL path").0
    )
}

async fn tombstone(store: &PostgresArtifactStore, artifact: &ArtifactRef, owner: &str) {
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new(format!("host:{owner}")).unwrap(),
            policy: ArtifactRetentionPolicy::new(format!("{owner}-test")).unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn postgres_protection_survives_processes_and_full_dump_restores_state_and_blobs_under_writes(
) {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool.clone());
    let artifact = upload(&store, b"postgres protected blob").await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    assert_protection_rejects_a_corrupt_blob(&store, &raw, &artifact).await;

    let key = ArtifactIdempotencyKey::new("backup:postgres-container").unwrap();
    let snapshot = store.protect_snapshot(key.clone()).await.unwrap();
    assert_eq!(snapshot.records.len(), 1);
    let reopened = reconnect(&url).await;
    assert_snapshot_survives_reopen(&reopened, &key, &snapshot).await;
    assert_dump_under_writes_restores_state_and_blobs(
        &container, &raw, &url, &artifact, &key, &snapshot,
    )
    .await;
    assert_released_snapshot_key_is_spent(&reopened, key).await;
    assert_gc_plan_yields_to_a_later_protection(&reopened, &artifact).await;
    assert_commit_wins_its_race_against_gc(&reopened).await;
}

async fn assert_protection_rejects_a_corrupt_blob(
    store: &PostgresArtifactStore,
    raw: &sqlx::PgPool,
    artifact: &ArtifactRef,
) {
    let overwrite = |bytes: Vec<u8>| {
        sqlx::query("UPDATE artifact_blobs SET bytes = $2 WHERE digest = $1")
            .bind(artifact.digest().as_str())
            .bind(bytes)
            .execute(raw)
    };
    overwrite(vec![0_u8]).await.unwrap();
    assert_eq!(
        store
            .protect_references(
                ArtifactIdempotencyKey::new("receipt:corrupt-postgres").unwrap(),
                vec![artifact.artifact_id().clone()],
            )
            .await,
        Err(ArtifactStoreError::FinalDigestMismatch)
    );
    overwrite(b"postgres protected blob".to_vec())
        .await
        .unwrap();
}

async fn assert_snapshot_survives_reopen(
    reopened: &PostgresArtifactStore,
    key: &ArtifactIdempotencyKey,
    snapshot: &ArtifactSnapshot,
) {
    assert_eq!(
        &reopened.protect_snapshot(key.clone()).await.unwrap(),
        snapshot
    );
    assert_eq!(
        reopened.protect_references(key.clone(), Vec::new()).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let preview_now = OffsetDateTime::now_utc();
    let protected_preview = reopened
        .plan_gc(
            preview_now,
            maintenance_lease("postgres-preview", "postgres-preview", preview_now),
        )
        .await
        .unwrap();
    assert!(protected_preview.exclusions.iter().any(|excluded| {
        excluded
            .reasons
            .contains(&ArtifactGcExclusionReason::LiveReference)
            && excluded
                .reasons
                .contains(&ArtifactGcExclusionReason::ProtectedReference)
    }));
}

async fn assert_dump_under_writes_restores_state_and_blobs(
    container: &PostgresContainer,
    raw: &sqlx::PgPool,
    url: &str,
    artifact: &ArtifactRef,
    key: &ArtifactIdempotencyKey,
    snapshot: &ArtifactSnapshot,
) {
    sqlx::query(
        "CREATE TABLE backup_writer (position BIGSERIAL PRIMARY KEY, payload TEXT NOT NULL)",
    )
    .execute(raw)
    .await
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let writer_pool = sqlx::PgPool::connect(url).await.unwrap();
    let writer = tokio::spawn(async move {
        while !writer_stop.load(Ordering::Acquire) {
            sqlx::query("INSERT INTO backup_writer(payload) VALUES ('during-dump')")
                .execute(&writer_pool)
                .await
                .unwrap();
        }
    });
    exec_ok(
        container,
        [
            "pg_dump",
            "-U",
            "made",
            "-Fc",
            "-f",
            "/tmp/protected.dump",
            "made",
        ],
    )
    .await;
    stop.store(true, Ordering::Release);
    writer.await.unwrap();
    exec_ok(container, ["createdb", "-U", "made", "made_restore"]).await;
    exec_ok(
        container,
        [
            "pg_restore",
            "-U",
            "made",
            "-d",
            "made_restore",
            "/tmp/protected.dump",
        ],
    )
    .await;

    let restore_url = sibling_database_url(url, "made_restore");
    let restored_raw = sqlx::PgPool::connect(&restore_url).await.unwrap();
    let restored = reconnect(&restore_url).await;
    assert_eq!(
        &restored.get(artifact.artifact_id()).await.unwrap().artifact,
        artifact
    );
    assert_eq!(
        &restored.protect_snapshot(key.clone()).await.unwrap(),
        snapshot
    );
    let blob_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(OCTET_LENGTH(bytes)), 0) FROM artifact_blobs WHERE digest = $1",
    )
    .bind(artifact.digest().as_str())
    .fetch_one(&restored_raw)
    .await
    .unwrap();
    assert_eq!(blob_bytes, artifact.size_bytes().get() as i64);
}

/// Releasing is idempotent, and a released key cannot protect again.
async fn assert_released_snapshot_key_is_spent(
    reopened: &PostgresArtifactStore,
    key: ArtifactIdempotencyKey,
) {
    reopened.release_snapshot(&key).await.unwrap();
    reopened.release_snapshot(&key).await.unwrap();
    assert_eq!(
        reopened.protect_snapshot(key).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
}

/// A GC plan computed before a protection was taken must not collect the
/// protected blob; once the protection is released the same plan applies.
async fn assert_gc_plan_yields_to_a_later_protection(
    reopened: &PostgresArtifactStore,
    artifact: &ArtifactRef,
) {
    tombstone(reopened, artifact, "postgres-gc").await;
    let now = OffsetDateTime::now_utc();
    let plan = reopened
        .plan_gc(
            now,
            maintenance_lease("postgres-maintenance", "postgres-gc-plan", now),
        )
        .await
        .unwrap();
    assert_eq!(plan.candidates.len(), 1);
    let stale_key = ArtifactIdempotencyKey::new("receipt:postgres-stale-gc").unwrap();
    reopened
        .protect_references(stale_key.clone(), vec![artifact.artifact_id().clone()])
        .await
        .unwrap();
    assert_eq!(
        reopened.apply_gc(&plan, OffsetDateTime::now_utc()).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    reopened.release_snapshot(&stale_key).await.unwrap();
    let report = reopened
        .apply_gc(&plan, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert_eq!(report.deleted, vec![artifact.digest().clone()]);
    assert_eq!(report.reclaimed_bytes, artifact.size_bytes().get());
    assert!(!reopened
        .backup_content_available(artifact.artifact_id())
        .await
        .unwrap());
}

/// A commit that revives a digest the GC plan meant to collect wins: the GC
/// application conflicts and the committed content stays readable.
async fn assert_commit_wins_its_race_against_gc(reopened: &PostgresArtifactStore) {
    let racing_bytes = b"postgres commit versus gc";
    let retired = upload(reopened, racing_bytes).await;
    tombstone(reopened, &retired, "postgres-gc-race").await;
    let race_now = OffsetDateTime::now_utc();
    let race_plan = reopened
        .plan_gc(
            race_now,
            maintenance_lease("postgres-gc-race", "postgres-gc-race-plan", race_now),
        )
        .await
        .unwrap();
    assert_eq!(race_plan.candidates.len(), 1);
    let live_id = ArtifactId::new("postgres-racing-live-reference").unwrap();
    let racing_upload = reopened
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: Some(live_id.clone()),
            expected_digest: digest(racing_bytes),
            size_bytes: ArtifactSizeBytes::new(racing_bytes.len() as u64),
            media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
            provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
            idempotency_key: ArtifactIdempotencyKey::new("postgres-gc-racing-upload").unwrap(),
        })
        .await
        .unwrap();
    reopened
        .put_chunk(PutArtifactChunk {
            upload_id: racing_upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: racing_bytes.to_vec(),
            chunk_digest: digest(racing_bytes),
        })
        .await
        .unwrap();
    let (committed, collected) = tokio::join!(
        reopened.commit_upload(&racing_upload.upload_id),
        reopened.apply_gc(&race_plan, OffsetDateTime::now_utc())
    );
    let committed = committed.unwrap();
    assert_eq!(committed.artifact_id(), &live_id);
    assert_eq!(collected, Err(ArtifactStoreError::IdempotencyConflict));
    assert_eq!(
        reopened.get(&live_id).await.unwrap().artifact.digest(),
        &digest(racing_bytes)
    );
    assert!(reopened.backup_content_available(&live_id).await.unwrap());
}

async fn assert_receipt_rejects_collected_content(
    store: &PostgresArtifactStore,
    artifact_service: &ArtifactService,
) {
    let collected_bytes = b"receipt bytes collected by postgres gc";
    let (collected_operation, _, collected_fence) = operation_fixture("postgres-receipt-collected");
    let collected = upload_receipt_artifact(
        store,
        collected_bytes,
        &collected_operation,
        &collected_fence,
    )
    .await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: collected.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("postgres-receipt-gc").unwrap(),
            policy: ArtifactRetentionPolicy::new("postgres-receipt-gc").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(1);
    let collect_lease = StepLease::acquire(
        LeaseOwnerId::new("postgres-receipt-collector").unwrap(),
        IdempotencyKey::new("postgres-receipt-collector").unwrap(),
        OffsetDateTime::now_utc(),
        DurationMs::from_millis(300_000),
    )
    .unwrap();
    let collect_plan = store.plan_gc(cutoff, collect_lease).await.unwrap();
    store.apply_gc(&collect_plan, cutoff).await.unwrap();
    let collected_receipt = receipt_fixture(&collected_operation, collected_fence, collected);
    assert!(artifact_service
        .protect_execution_receipt(&collected_receipt)
        .await
        .is_err());
}

#[tokio::test]
async fn postgres_receipt_requires_content_and_survives_reopen() {
    let (pool, url, _container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool.clone());
    let artifact_service = ArtifactService::new(Arc::new(store.clone()));
    assert_receipt_rejects_collected_content(&store, &artifact_service).await;

    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(1);
    let protected_bytes = b"receipt bytes protected by postgres receipt";
    let (operation, intent, claim_fence) = operation_fixture("postgres-receipt-content");
    let protected =
        upload_receipt_artifact(&store, protected_bytes, &operation, &claim_fence).await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: protected.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("postgres-receipt-pin").unwrap(),
            policy: ArtifactRetentionPolicy::new("postgres-receipt-pin").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let pin_lease = StepLease::acquire(
        LeaseOwnerId::new("postgres-receipt-pin-plan").unwrap(),
        IdempotencyKey::new("postgres-receipt-pin-plan").unwrap(),
        OffsetDateTime::now_utc(),
        DurationMs::from_millis(300_000),
    )
    .unwrap();
    let stale_plan = store.plan_gc(cutoff, pin_lease).await.unwrap();
    let receipt = receipt_fixture(&operation, claim_fence, protected.clone());
    let ceremony = PostgresCeremonyStore::new(pool);
    ceremony.record_intent(intent).await.unwrap();
    artifact_service
        .protect_execution_receipt(&receipt)
        .await
        .unwrap();
    ceremony.record_receipt(receipt.clone()).await.unwrap();
    assert_eq!(
        ceremony.receipt(operation.operation_id()).await.unwrap(),
        Some(receipt.clone())
    );

    let reopened_pool = PostgresPool::connect(&PostgresConfig::from_url(url.clone()))
        .await
        .unwrap();
    let reopened = PostgresArtifactStore::new(reopened_pool.clone());
    let reopened_ceremony = PostgresCeremonyStore::new(reopened_pool);
    assert_eq!(
        reopened_ceremony
            .receipt(operation.operation_id())
            .await
            .unwrap(),
        Some(receipt.clone())
    );
    let reopened_record = reopened.get(protected.artifact_id()).await.unwrap();
    let active = reopened.active_protections().await.unwrap();
    assert!(active.iter().any(|snapshot| {
        snapshot.key.as_str() == format!("receipt:{}", receipt.receipt_id())
            && snapshot.records == vec![reopened_record.clone()]
    }));
    assert_eq!(
        reopened
            .apply_gc(&stale_plan, OffsetDateTime::now_utc())
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    assert!(reopened
        .plan_gc(
            cutoff,
            StepLease::acquire(
                LeaseOwnerId::new("postgres-receipt-reopen").unwrap(),
                IdempotencyKey::new("postgres-receipt-reopen").unwrap(),
                OffsetDateTime::UNIX_EPOCH,
                DurationMs::from_millis(300_000),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .candidates
        .is_empty());
    let page = reopened
        .read_chunk_for_backup(ReadArtifactChunk {
            artifact_id: protected.artifact_id().clone(),
            offset: ArtifactByteOffset::ZERO,
            max_bytes: ArtifactChunkLimit::default(),
        })
        .await
        .unwrap();
    assert_eq!(reopened_record.artifact, protected);
    assert_eq!(
        reopened_record.artifact.size_bytes().get(),
        protected_bytes.len() as u64
    );
    assert_eq!(reopened_record.artifact.digest(), &digest(protected_bytes));
    assert_eq!(page.bytes, protected_bytes);
    assert_eq!(page.chunk_digest, digest(protected_bytes));
    assert!(page.is_complete());
}

#[cfg(unix)]
fn write_executable(path: &std::path::Path, script: &str) {
    std::fs::write(path, script).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
async fn has_active_protection(
    store: &PostgresArtifactStore,
    key: &ArtifactIdempotencyKey,
) -> bool {
    store
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| &protection.key == key)
}

/// A receipt whose artifact is pinned before the backup, so the restored
/// database must carry both the receipt and its protection.
#[cfg(unix)]
struct PinnedReceipt {
    operation: ExecutionOperation,
    artifact: ArtifactRef,
    receipt: ExecutionReceipt,
}

#[cfg(unix)]
impl PinnedReceipt {
    async fn record(store: &PostgresArtifactStore, pool: PostgresPool) -> Self {
        let (operation, intent, fence) = operation_fixture("postgres-backup-restored-receipt");
        let artifact = upload_receipt_artifact(
            store,
            b"receipt pin that must survive postgres restore",
            &operation,
            &fence,
        )
        .await;
        let receipt = receipt_fixture(&operation, fence, artifact.clone());
        let ceremony = PostgresCeremonyStore::new(pool);
        ceremony.record_intent(intent).await.unwrap();
        ArtifactService::new(Arc::new(store.clone()))
            .protect_execution_receipt(&receipt)
            .await
            .unwrap();
        ceremony.record_receipt(receipt.clone()).await.unwrap();
        Self {
            operation,
            artifact,
            receipt,
        }
    }
}

/// `pg_dump` / `pg_restore` stand-ins that run the real clients inside the
/// container. The dump announces itself and then holds until released, so
/// the test can prove writers commit while it reads its exported snapshot.
#[cfg(unix)]
struct HeldDumpClients {
    dump: std::path::PathBuf,
    restore: std::path::PathBuf,
    dump_started: std::path::PathBuf,
    release_dump: std::path::PathBuf,
}

#[cfg(unix)]
impl HeldDumpClients {
    fn write(scratch: &std::path::Path, container: &PostgresContainer) -> Self {
        let clients = Self {
            dump: scratch.join("pg_dump-wrapper"),
            restore: scratch.join("pg_restore-wrapper"),
            dump_started: scratch.join("pg-dump-started"),
            release_dump: scratch.join("release-pg-dump"),
        };
        write_executable(
            &clients.dump,
            &format!(
                "#!/bin/sh\nset -eu\nout=''\nsnapshot=''\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    --file) shift; out=$1 ;;\n    --snapshot) shift; snapshot=$1 ;;\n  esac\n  shift || true\ndone\ntest -n \"$snapshot\"\nstarted='{}'\nrelease='{}'\ntouch \"$started\"\nwhile [ ! -f \"$release\" ]; do sleep 0.01; done\ndocker exec {} pg_dump -U made -Fc --snapshot \"$snapshot\" -f /tmp/service.dump made\ndocker cp {}:/tmp/service.dump \"$out\" >/dev/null\n",
                clients.dump_started.display(),
                clients.release_dump.display(),
                container.id(),
                container.id()
            ),
        );
        write_executable(
            &clients.restore,
            &format!(
                "#!/bin/sh\nset -eu\nif [ \"$1\" = '--list' ]; then\n  archive=$2\n  docker cp \"$archive\" {}:/tmp/service.dump >/dev/null\n  docker exec {} pg_restore --list /tmp/service.dump >/dev/null\nelse\n  for archive do :; done\n  docker cp \"$archive\" {}:/tmp/service.dump >/dev/null\n  docker exec {} pg_restore -U made -d made_service_restore /tmp/service.dump\nfi\n",
                container.id(),
                container.id(),
                container.id(),
                container.id()
            ),
        );
        clients
    }
}

/// A SQL writer and an artifact writer that keep committing until stopped.
#[cfg(unix)]
struct ConcurrentWriters {
    stop: Arc<AtomicBool>,
    sql_writes: Arc<AtomicUsize>,
    artifact_mutations: Arc<AtomicUsize>,
    handles: [tokio::task::JoinHandle<()>; 2],
}

#[cfg(unix)]
impl ConcurrentWriters {
    async fn start(raw: &sqlx::PgPool, store: &PostgresArtifactStore) -> Self {
        sqlx::query("CREATE TABLE service_writer(position BIGSERIAL PRIMARY KEY, committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp())")
            .execute(raw)
            .await
            .unwrap();
        sqlx::query("INSERT INTO service_writer DEFAULT VALUES")
            .execute(raw)
            .await
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let sql_writes = Arc::new(AtomicUsize::new(0));
        let artifact_mutations = Arc::new(AtomicUsize::new(0));
        let sql_task = tokio::spawn(Self::write_sql(
            raw.clone(),
            stop.clone(),
            sql_writes.clone(),
        ));
        let artifact_task = tokio::spawn(Self::write_artifacts(
            store.clone(),
            stop.clone(),
            artifact_mutations.clone(),
        ));
        Self {
            stop,
            sql_writes,
            artifact_mutations,
            handles: [sql_task, artifact_task],
        }
    }

    async fn write_sql(pool: sqlx::PgPool, stop: Arc<AtomicBool>, writes: Arc<AtomicUsize>) {
        while !stop.load(Ordering::Acquire) {
            sqlx::query("INSERT INTO service_writer DEFAULT VALUES")
                .execute(&pool)
                .await
                .unwrap();
            writes.fetch_add(1, Ordering::Release);
        }
    }

    async fn write_artifacts(
        store: PostgresArtifactStore,
        stop: Arc<AtomicBool>,
        mutations: Arc<AtomicUsize>,
    ) {
        let mut sequence = 0_u64;
        while !stop.load(Ordering::Acquire) {
            let bytes = format!("concurrent-postgres-artifact-{sequence}").into_bytes();
            let concurrent = upload(&store, &bytes).await;
            if sequence.is_multiple_of(2) {
                store
                    .tombstone(TombstoneArtifact {
                        artifact_id: concurrent.artifact_id().clone(),
                        actor: ArtifactRetentionActor::new("host:postgres-backup-writer").unwrap(),
                        policy: ArtifactRetentionPolicy::new("postgres-backup-snapshot-test")
                            .unwrap(),
                        retired_at: OffsetDateTime::now_utc(),
                    })
                    .await
                    .unwrap();
            }
            mutations.fetch_add(1, Ordering::Release);
            sequence += 1;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    fn sql_writes(&self) -> usize {
        self.sql_writes.load(Ordering::Acquire)
    }

    async fn until(&self, what: &str, condition: impl Fn(&Self) -> bool) {
        tokio::time::timeout(scaled_budget(std::time::Duration::from_secs(5)), async {
            while !condition(self) {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{what}"));
    }

    async fn stop(self) {
        self.stop.store(true, Ordering::Release);
        for handle in self.handles {
            handle.await.unwrap();
        }
    }
}

/// What the backup under writers observed, for the assertions and the
/// RPO/RTO sample.
#[cfg(unix)]
struct BackupUnderWriters {
    manifest: PostgresBackupManifest,
    duration: std::time::Duration,
    sql_writes_before: usize,
    sql_writes_during: usize,
    sql_writes_total: usize,
}

/// Runs `backup_to` while both writers commit, and proves a SQL commit lands
/// while `pg_dump` is held on its exported snapshot.
#[cfg(unix)]
async fn backup_under_writers(
    service: &PostgresBackupService,
    backup: &std::path::Path,
    clients: &HeldDumpClients,
    writers: ConcurrentWriters,
) -> BackupUnderWriters {
    writers
        .until("writers must start before the backup", |writers| {
            writers.artifact_mutations.load(Ordering::Acquire) >= 2 && writers.sql_writes() > 0
        })
        .await;
    let sql_writes_before = writers.sql_writes();
    let started = std::time::Instant::now();
    // backup_to runs pg_dump as a blocking child process, so it needs its own
    // thread to keep the writers running while the dump is held. It must still
    // drive the future on the test runtime: the service shares the store's
    // pool, and a connection the pool opens under a second runtime stays bound
    // to that runtime's I/O driver. Once that runtime is dropped, the writers
    // that reuse the connection fail with "Tokio context ... being shutdown".
    let runtime = tokio::runtime::Handle::current();
    let key = ArtifactIdempotencyKey::new("backup:postgres-service").unwrap();
    let backup_task = tokio::task::spawn_blocking({
        let service = service.clone();
        let backup = backup.to_path_buf();
        move || runtime.block_on(service.backup_to(backup, key))
    });
    writers
        .until(
            "pg_dump must start under the exported transaction snapshot",
            |_| {
                assert!(
                    !backup_task.is_finished(),
                    "PostgreSQL backup completed before reaching the dump barrier"
                );
                clients.dump_started.exists()
            },
        )
        .await;
    let at_dump_barrier = writers.sql_writes();
    writers
        .until(
            "SQL writer must commit while pg_dump is held on its snapshot",
            |writers| writers.sql_writes() > at_dump_barrier,
        )
        .await;
    assert!(
        !backup_task.is_finished(),
        "the observed SQL commit must precede backup completion"
    );
    let sql_writes_during = writers.sql_writes() - at_dump_barrier;
    std::fs::write(&clients.release_dump, b"continue").unwrap();
    let result = backup_task.await.unwrap();
    let duration = started.elapsed();
    let sql_writes = writers.sql_writes.clone();
    writers.stop().await;
    BackupUnderWriters {
        manifest: result.unwrap(),
        duration,
        sql_writes_before,
        sql_writes_during,
        sql_writes_total: sql_writes.load(Ordering::Acquire),
    }
}

/// Row count and last commit time (ms) of `service_writer`.
#[cfg(unix)]
async fn writer_frontier(pool: &sqlx::PgPool) -> (i64, i64) {
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_writer")
        .fetch_one(pool)
        .await
        .unwrap();
    let last_ms: i64 = sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM MAX(committed_at)) * 1000)::BIGINT FROM service_writer",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (rows, last_ms)
}

/// Two restores reach a newly-created destination at once. The
/// destination-scoped advisory lock makes one complete and makes the waiter
/// observe the now non-empty target as an idempotency conflict.
#[cfg(unix)]
async fn race_two_restores(
    service: &PostgresBackupService,
    backup: &std::path::Path,
    target_url: &str,
) -> std::time::Duration {
    let started = std::time::Instant::now();
    let (left, right) = tokio::join!(
        service.restore_to(backup, target_url),
        service.restore_to(backup, target_url),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(
        (left, right),
        (Ok(()), Err(ArtifactStoreError::IdempotencyConflict))
            | (Err(ArtifactStoreError::IdempotencyConflict), Ok(()))
    ));
    started.elapsed()
}

#[cfg(unix)]
async fn assert_target_holds_the_backup(
    target_url: &str,
    manifest: &PostgresBackupManifest,
    artifact: &ArtifactRef,
    pinned: &PinnedReceipt,
) {
    let target_pool = PostgresPool::connect(&PostgresConfig::from_url(target_url.to_owned()))
        .await
        .unwrap();
    let target = PostgresArtifactStore::new(target_pool.clone());
    assert_eq!(
        &target.get(artifact.artifact_id()).await.unwrap().artifact,
        artifact
    );
    assert_eq!(
        target
            .list(None, ArtifactPageLimit::new(100).unwrap())
            .await
            .unwrap()
            .items,
        manifest.artifact_records
    );
    for record in &manifest.artifact_records {
        let restored = target
            .read_chunk_for_backup(ReadArtifactChunk {
                artifact_id: record.artifact.artifact_id().clone(),
                offset: ArtifactByteOffset::ZERO,
                max_bytes: ArtifactChunkLimit::default(),
            })
            .await
            .unwrap()
            .bytes;
        assert_eq!(restored.len() as u64, record.artifact.size_bytes().get());
        assert_eq!(&digest(&restored), record.artifact.digest());
    }
    assert!(!manifest.snapshot_id.is_empty());
    assert!(!manifest.transaction_snapshot.is_empty());
    assert!(target
        .backup_content_available(artifact.artifact_id())
        .await
        .unwrap());
    assert_eq!(
        PostgresCeremonyStore::new(target_pool)
            .receipt(pinned.operation.operation_id())
            .await
            .unwrap(),
        Some(pinned.receipt.clone())
    );
    assert!(target
        .backup_content_available(pinned.artifact.artifact_id())
        .await
        .unwrap());
    assert!(target
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| {
            protection.key.as_str() == format!("receipt:{}", pinned.receipt.receipt_id())
                && protection
                    .records
                    .iter()
                    .any(|record| record.artifact == pinned.artifact)
        }));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_backup_service_verifies_archive_and_restores_only_to_empty_database() {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool.clone());
    let artifact = upload(&store, b"service archive boundary").await;
    let pinned = PinnedReceipt::record(&store, pool).await;
    let scratch = tempfile::TempDir::new().unwrap();
    let clients = HeldDumpClients::write(scratch.path(), &container);
    let service = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&clients.dump, &clients.restore);
    let backup = scratch.path().join("backup");
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    let writers = ConcurrentWriters::start(&raw, &store).await;
    let observed = backup_under_writers(&service, &backup, &clients, writers).await;
    let manifest = &observed.manifest;
    let (source_rows, source_last_ms) = writer_frontier(&raw).await;
    assert_eq!(
        source_rows,
        1 + i64::try_from(observed.sql_writes_total).unwrap(),
        "the SQL commit counter must describe the source frontier"
    );

    service.verify(&backup, manifest).unwrap();
    assert_eq!(
        service.restore_to(&backup, &url).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    assert!(!format!("{service:?}").contains(&url));
    exec_ok(
        &container,
        ["createdb", "-U", "made", "made_service_restore"],
    )
    .await;
    let target_url = sibling_database_url(&url, "made_service_restore");
    let restore_duration = race_two_restores(&service, &backup, &target_url).await;
    assert_target_holds_the_backup(&target_url, manifest, &artifact, &pinned).await;
    assert_eq!(
        service.restore_to(&backup, &target_url).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let target_raw = sqlx::PgPool::connect(&target_url).await.unwrap();
    let (restored_rows, restored_last_ms) = writer_frontier(&target_raw).await;
    assert!(
        restored_rows > i64::try_from(observed.sql_writes_before).unwrap()
            && restored_rows <= source_rows,
        "backup must contain the pre-barrier SQL frontier and no future rows"
    );
    eprintln!(
        "{}",
        serde_json::json!({
            "backend":"postgres",
            "sample":"backup_restore_under_writers_with_destination_fence",
            "archive_digest":manifest.archive_digest,
            "backup_ms":observed.duration.as_secs_f64()*1000.0,
            "restore_ms":restore_duration.as_secs_f64()*1000.0,
            "rto_ms":restore_duration.as_secs_f64()*1000.0,
            "source_frontier":source_rows,
            "restored_frontier":restored_rows,
            "sql_writes_before_backup":observed.sql_writes_before,
            "sql_writes_during_backup":observed.sql_writes_during,
            "rpo_missing_rows":source_rows-restored_rows,
            "rpo_lag_ms":source_last_ms-restored_last_ms,
            "receipt_pin":pinned.receipt.receipt_id(),
            "restore_race":{"successes":1,"conflicts":1}
        })
    );
}

#[cfg(unix)]
#[tokio::test]
async fn postgres_backup_client_failures_keep_the_pin_and_never_publish_a_manifest() {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool);
    upload(&store, b"client failure recovery").await;
    let scratch = tempfile::TempDir::new().unwrap();
    let dump = scratch.path().join("pg_dump-wrapper");
    let restore = scratch.path().join("pg_restore-wrapper");
    let fail_dump = scratch.path().join("pg_dump-fail");
    let fail_verify = scratch.path().join("pg_restore-list-fail");
    write_executable(
        &dump,
        &format!(
            "#!/bin/sh\nset -eu\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = '--file' ]; then shift; out=$1; fi\n  shift || true\ndone\ndocker exec {} rm -f /tmp/fault.dump\ndocker exec {} pg_dump -U made -Fc -f /tmp/fault.dump made\ndocker cp {}:/tmp/fault.dump \"$out\" >/dev/null\n",
            container.id(),
            container.id(),
            container.id()
        ),
    );
    write_executable(
        &restore,
        &format!(
            "#!/bin/sh\nset -eu\narchive=$2\ndocker cp \"$archive\" {}:/tmp/fault.dump >/dev/null\ndocker exec {} pg_restore --list /tmp/fault.dump >/dev/null\n",
            container.id(),
            container.id()
        ),
    );
    write_executable(&fail_dump, "#!/bin/sh\nexit 17\n");
    write_executable(&fail_verify, "#!/bin/sh\nexit 19\n");

    let dump_failure = scratch.path().join("dump-failure");
    let dump_key = ArtifactIdempotencyKey::new("backup:postgres-dump-failure").unwrap();
    let failing_dump = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&fail_dump, &restore);
    assert_eq!(
        failing_dump
            .backup_to(&dump_failure, dump_key.clone())
            .await,
        Err(ArtifactStoreError::InvalidBackup)
    );
    assert!(!dump_failure.join("postgres-manifest.json").exists());
    assert!(has_active_protection(&store, &dump_key).await);

    let healthy = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&dump, &restore);
    healthy
        .backup_to(&dump_failure, dump_key.clone())
        .await
        .unwrap();
    assert!(dump_failure.join("postgres-manifest.json").exists());
    assert!(!has_active_protection(&store, &dump_key).await);

    let verify_failure = scratch.path().join("verify-failure");
    let verify_key = ArtifactIdempotencyKey::new("backup:postgres-verify-failure").unwrap();
    let failing_verify = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&dump, &fail_verify);
    assert_eq!(
        failing_verify
            .backup_to(&verify_failure, verify_key.clone())
            .await,
        Err(ArtifactStoreError::InvalidBackup)
    );
    assert!(!verify_failure.join("postgres-manifest.json").exists());
    assert!(verify_failure.join("database.dump").exists());
    assert!(has_active_protection(&store, &verify_key).await);
    let owner_before: serde_json::Value =
        serde_json::from_slice(&std::fs::read(verify_failure.join("postgres-owner.json")).unwrap())
            .unwrap();
    let retry_without_dump = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&fail_dump, &restore);
    let retry_manifest = retry_without_dump
        .backup_to(&verify_failure, verify_key.clone())
        .await
        .unwrap();
    assert!(verify_failure.join("postgres-manifest.json").exists());
    assert_eq!(
        retry_manifest.snapshot_id,
        owner_before["snapshot_id"].as_str().unwrap()
    );
    assert!(!has_active_protection(&store, &verify_key).await);
}

async fn exec_ok<const N: usize>(container: &PostgresContainer, command: [&str; N]) {
    let result = container
        .exec(ExecCommand::new(command).with_cmd_ready_condition(CmdWaitFor::exit_code(0)))
        .await
        .unwrap();
    assert_eq!(result.exit_code().await.unwrap(), Some(0));
}
