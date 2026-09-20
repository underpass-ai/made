#![cfg(feature = "container-tests")]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use made_adapters::artifacts::ArtifactGcExclusionReason;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresBackupService, PostgresCeremonyStore, PostgresConfig,
    PostgresPool,
};
use made_app::artifacts::ArtifactService;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, ExecutionReceiptStorePort, PutArtifactChunk, ReadArtifactChunk,
    TombstoneArtifact,
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

#[tokio::test]
async fn postgres_protection_survives_processes_and_full_dump_restores_state_and_blobs_under_writes(
) {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool.clone());
    let artifact = upload(&store, b"postgres protected blob").await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("UPDATE artifact_blobs SET bytes = $2 WHERE digest = $1")
        .bind(artifact.digest().as_str())
        .bind(vec![0_u8])
        .execute(&raw)
        .await
        .unwrap();
    assert_eq!(
        store
            .protect_references(
                ArtifactIdempotencyKey::new("receipt:corrupt-postgres").unwrap(),
                vec![artifact.artifact_id().clone()],
            )
            .await,
        Err(ArtifactStoreError::FinalDigestMismatch)
    );
    sqlx::query("UPDATE artifact_blobs SET bytes = $2 WHERE digest = $1")
        .bind(artifact.digest().as_str())
        .bind(b"postgres protected blob".as_slice())
        .execute(&raw)
        .await
        .unwrap();
    let key = ArtifactIdempotencyKey::new("backup:postgres-container").unwrap();
    let snapshot = store.protect_snapshot(key.clone()).await.unwrap();
    assert_eq!(snapshot.records.len(), 1);

    let reopened = PostgresArtifactStore::new(
        PostgresPool::connect(&PostgresConfig::from_url(url.clone()))
            .await
            .unwrap(),
    );
    assert_eq!(
        reopened.protect_snapshot(key.clone()).await.unwrap(),
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
            StepLease::acquire(
                LeaseOwnerId::new("postgres-preview").unwrap(),
                IdempotencyKey::new("postgres-preview").unwrap(),
                preview_now,
                DurationMs::from_millis(300_000),
            )
            .unwrap(),
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

    sqlx::query(
        "CREATE TABLE backup_writer (position BIGSERIAL PRIMARY KEY, payload TEXT NOT NULL)",
    )
    .execute(&raw)
    .await
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let writer_pool = sqlx::PgPool::connect(&url).await.unwrap();
    let writer = tokio::spawn(async move {
        while !writer_stop.load(Ordering::Acquire) {
            sqlx::query("INSERT INTO backup_writer(payload) VALUES ('during-dump')")
                .execute(&writer_pool)
                .await
                .unwrap();
        }
    });
    exec_ok(
        &container,
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
    exec_ok(&container, ["createdb", "-U", "made", "made_restore"]).await;
    exec_ok(
        &container,
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

    let restore_url = format!(
        "{}/made_restore",
        url.rsplit_once('/').expect("database URL path").0
    );
    let restored_raw = sqlx::PgPool::connect(&restore_url).await.unwrap();
    let restored_pool = PostgresPool::connect(&PostgresConfig::from_url(restore_url))
        .await
        .unwrap();
    let restored = PostgresArtifactStore::new(restored_pool.clone());
    assert_eq!(
        restored.get(artifact.artifact_id()).await.unwrap().artifact,
        artifact
    );
    assert_eq!(
        restored.protect_snapshot(key.clone()).await.unwrap(),
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

    reopened.release_snapshot(&key).await.unwrap();
    reopened.release_snapshot(&key).await.unwrap();
    assert_eq!(
        reopened.protect_snapshot(key).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );

    reopened
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:postgres-gc").unwrap(),
            policy: ArtifactRetentionPolicy::new("postgres-gc-test").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let now = OffsetDateTime::now_utc();
    let plan = reopened
        .plan_gc(
            now,
            StepLease::acquire(
                LeaseOwnerId::new("postgres-maintenance").unwrap(),
                IdempotencyKey::new("postgres-gc-plan").unwrap(),
                now,
                DurationMs::from_millis(300_000),
            )
            .unwrap(),
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

    let racing_bytes = b"postgres commit versus gc";
    let retired = upload(&reopened, racing_bytes).await;
    reopened
        .tombstone(TombstoneArtifact {
            artifact_id: retired.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:postgres-gc-race").unwrap(),
            policy: ArtifactRetentionPolicy::new("postgres-gc-race-test").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let race_now = OffsetDateTime::now_utc();
    let race_plan = reopened
        .plan_gc(
            race_now,
            StepLease::acquire(
                LeaseOwnerId::new("postgres-gc-race").unwrap(),
                IdempotencyKey::new("postgres-gc-race-plan").unwrap(),
                race_now,
                DurationMs::from_millis(300_000),
            )
            .unwrap(),
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_backup_service_verifies_archive_and_restores_only_to_empty_database() {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool.clone());
    let artifact = upload(&store, b"service archive boundary").await;
    let (receipt_operation, receipt_intent, receipt_fence) =
        operation_fixture("postgres-backup-restored-receipt");
    let receipt_artifact = upload_receipt_artifact(
        &store,
        b"receipt pin that must survive postgres restore",
        &receipt_operation,
        &receipt_fence,
    )
    .await;
    let receipt = receipt_fixture(&receipt_operation, receipt_fence, receipt_artifact.clone());
    let artifact_service = ArtifactService::new(Arc::new(store.clone()));
    let source_ceremony = PostgresCeremonyStore::new(pool.clone());
    source_ceremony.record_intent(receipt_intent).await.unwrap();
    artifact_service
        .protect_execution_receipt(&receipt)
        .await
        .unwrap();
    source_ceremony
        .record_receipt(receipt.clone())
        .await
        .unwrap();
    let scratch = tempfile::TempDir::new().unwrap();
    let dump = scratch.path().join("pg_dump-wrapper");
    let restore = scratch.path().join("pg_restore-wrapper");
    let dump_started = scratch.path().join("pg-dump-started");
    let release_dump = scratch.path().join("release-pg-dump");
    std::fs::write(
        &dump,
        format!(
            "#!/bin/sh\nset -eu\nout=''\nsnapshot=''\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    --file) shift; out=$1 ;;\n    --snapshot) shift; snapshot=$1 ;;\n  esac\n  shift || true\ndone\ntest -n \"$snapshot\"\nstarted='{}'\nrelease='{}'\ntouch \"$started\"\nwhile [ ! -f \"$release\" ]; do sleep 0.01; done\ndocker exec {} pg_dump -U made -Fc --snapshot \"$snapshot\" -f /tmp/service.dump made\ndocker cp {}:/tmp/service.dump \"$out\" >/dev/null\n",
            dump_started.display(),
            release_dump.display(),
            container.id(),
            container.id()
        ),
    )
    .unwrap();
    std::fs::write(
        &restore,
        format!(
            "#!/bin/sh\nset -eu\nif [ \"$1\" = '--list' ]; then\n  archive=$2\n  docker cp \"$archive\" {}:/tmp/service.dump >/dev/null\n  docker exec {} pg_restore --list /tmp/service.dump >/dev/null\nelse\n  for archive do :; done\n  docker cp \"$archive\" {}:/tmp/service.dump >/dev/null\n  docker exec {} pg_restore -U made -d made_service_restore /tmp/service.dump\nfi\n",
            container.id(),
            container.id(),
            container.id(),
            container.id()
        ),
    )
    .unwrap();
    for program in [&dump, &restore] {
        let mut permissions = std::fs::metadata(program).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(program, permissions).unwrap();
    }

    let service = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&dump, &restore);
    let backup = scratch.path().join("backup");
    let key = ArtifactIdempotencyKey::new("backup:postgres-service").unwrap();
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("CREATE TABLE service_writer(position BIGSERIAL PRIMARY KEY, committed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp())")
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("INSERT INTO service_writer DEFAULT VALUES")
        .execute(&raw)
        .await
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let writer_pool = raw.clone();
    let sql_writes = Arc::new(AtomicUsize::new(0));
    let observed_sql_writes = sql_writes.clone();
    let writer = tokio::spawn(async move {
        while !writer_stop.load(Ordering::Acquire) {
            sqlx::query("INSERT INTO service_writer DEFAULT VALUES")
                .execute(&writer_pool)
                .await
                .unwrap();
            sql_writes.fetch_add(1, Ordering::Release);
        }
    });
    let artifact_writer_stop = stop.clone();
    let artifact_writer_store = store.clone();
    let artifact_mutations = Arc::new(AtomicUsize::new(0));
    let observed_artifact_mutations = artifact_mutations.clone();
    let artifact_writer = tokio::spawn(async move {
        let mut sequence = 0_u64;
        while !artifact_writer_stop.load(Ordering::Acquire) {
            let bytes = format!("concurrent-postgres-artifact-{sequence}").into_bytes();
            let concurrent = upload(&artifact_writer_store, &bytes).await;
            if sequence.is_multiple_of(2) {
                artifact_writer_store
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
            artifact_mutations.fetch_add(1, Ordering::Release);
            sequence += 1;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while observed_artifact_mutations.load(Ordering::Acquire) < 2
            || observed_sql_writes.load(Ordering::Acquire) == 0
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let sql_writes_before_backup = observed_sql_writes.load(Ordering::Acquire);
    let backup_started = std::time::Instant::now();
    let backup_task = std::thread::spawn({
        let service = service.clone();
        let backup = backup.clone();
        move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(service.backup_to(backup, key))
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !dump_started.exists() {
            assert!(
                !backup_task.is_finished(),
                "PostgreSQL backup completed before reaching the dump barrier"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("pg_dump must start under the exported transaction snapshot");
    let sql_writes_at_dump_barrier = observed_sql_writes.load(Ordering::Acquire);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while observed_sql_writes.load(Ordering::Acquire) <= sql_writes_at_dump_barrier {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("SQL writer must commit while pg_dump is held on its snapshot");
    assert!(
        !backup_task.is_finished(),
        "the observed SQL commit must precede backup completion"
    );
    let sql_writes_during_backup =
        observed_sql_writes.load(Ordering::Acquire) - sql_writes_at_dump_barrier;
    std::fs::write(&release_dump, b"continue").unwrap();
    let backup_result = backup_task.join().unwrap();
    let backup_duration = backup_started.elapsed();
    stop.store(true, Ordering::Release);
    writer.await.unwrap();
    artifact_writer.await.unwrap();
    let manifest = backup_result.unwrap();
    let source_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_writer")
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(
        source_rows,
        1 + i64::try_from(observed_sql_writes.load(Ordering::Acquire)).unwrap(),
        "the SQL commit counter must describe the source frontier"
    );
    let source_last_ms: i64 = sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM MAX(committed_at)) * 1000)::BIGINT FROM service_writer",
    )
    .fetch_one(&raw)
    .await
    .unwrap();
    service.verify(&backup, &manifest).unwrap();
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
    let target_url = format!("{}/made_service_restore", url.rsplit_once('/').unwrap().0);
    let restore_started = std::time::Instant::now();
    // Both invocations reach a newly-created destination at once. The
    // destination-scoped advisory lock makes one complete and makes the
    // waiter observe the now non-empty target as an idempotency conflict.
    let (left, right) = tokio::join!(
        service.restore_to(&backup, &target_url),
        service.restore_to(&backup, &target_url),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(
        (left, right),
        (Ok(()), Err(ArtifactStoreError::IdempotencyConflict))
            | (Err(ArtifactStoreError::IdempotencyConflict), Ok(()))
    ));
    let restore_duration = restore_started.elapsed();
    let target_pool = PostgresPool::connect(&PostgresConfig::from_url(target_url.clone()))
        .await
        .unwrap();
    let target = PostgresArtifactStore::new(target_pool.clone());
    let target_ceremony = PostgresCeremonyStore::new(target_pool);
    assert_eq!(
        target.get(artifact.artifact_id()).await.unwrap().artifact,
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
        target_ceremony
            .receipt(receipt_operation.operation_id())
            .await
            .unwrap(),
        Some(receipt.clone())
    );
    assert!(target
        .backup_content_available(receipt_artifact.artifact_id())
        .await
        .unwrap());
    assert!(target
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| {
            protection.key.as_str() == format!("receipt:{}", receipt.receipt_id())
                && protection
                    .records
                    .iter()
                    .any(|record| record.artifact == receipt_artifact)
        }));
    assert_eq!(
        service.restore_to(&backup, &target_url).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let target_raw = sqlx::PgPool::connect(&target_url).await.unwrap();
    let restored_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_writer")
        .fetch_one(&target_raw)
        .await
        .unwrap();
    let restored_last_ms: i64 = sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM MAX(committed_at)) * 1000)::BIGINT FROM service_writer",
    )
    .fetch_one(&target_raw)
    .await
    .unwrap();
    assert!(
        restored_rows > i64::try_from(sql_writes_before_backup).unwrap()
            && restored_rows <= source_rows,
        "backup must contain the pre-barrier SQL frontier and no future rows"
    );
    eprintln!(
        "{}",
        serde_json::json!({
            "backend":"postgres",
            "sample":"backup_restore_under_writers_with_destination_fence",
            "archive_digest":manifest.archive_digest,
            "backup_ms":backup_duration.as_secs_f64()*1000.0,
            "restore_ms":restore_duration.as_secs_f64()*1000.0,
            "rto_ms":restore_duration.as_secs_f64()*1000.0,
            "source_frontier":source_rows,
            "restored_frontier":restored_rows,
            "sql_writes_before_backup":sql_writes_before_backup,
            "sql_writes_during_backup":sql_writes_during_backup,
            "rpo_missing_rows":source_rows-restored_rows,
            "rpo_lag_ms":source_last_ms-restored_last_ms,
            "receipt_pin":receipt.receipt_id(),
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
    std::fs::write(
        &dump,
        format!(
            "#!/bin/sh\nset -eu\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = '--file' ]; then shift; out=$1; fi\n  shift || true\ndone\ndocker exec {} rm -f /tmp/fault.dump\ndocker exec {} pg_dump -U made -Fc -f /tmp/fault.dump made\ndocker cp {}:/tmp/fault.dump \"$out\" >/dev/null\n",
            container.id(),
            container.id(),
            container.id()
        ),
    )
    .unwrap();
    std::fs::write(
        &restore,
        format!(
            "#!/bin/sh\nset -eu\narchive=$2\ndocker cp \"$archive\" {}:/tmp/fault.dump >/dev/null\ndocker exec {} pg_restore --list /tmp/fault.dump >/dev/null\n",
            container.id(),
            container.id()
        ),
    )
    .unwrap();
    std::fs::write(&fail_dump, "#!/bin/sh\nexit 17\n").unwrap();
    std::fs::write(&fail_verify, "#!/bin/sh\nexit 19\n").unwrap();
    for program in [&dump, &restore, &fail_dump, &fail_verify] {
        let mut permissions = std::fs::metadata(program).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(program, permissions).unwrap();
    }

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
    assert!(store
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| protection.key == dump_key));

    let healthy = PostgresBackupService::new(store.clone(), url.clone())
        .with_client_programs(&dump, &restore);
    healthy
        .backup_to(&dump_failure, dump_key.clone())
        .await
        .unwrap();
    assert!(dump_failure.join("postgres-manifest.json").exists());
    assert!(!store
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| protection.key == dump_key));

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
    assert!(store
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| protection.key == verify_key));
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
    assert!(!store
        .active_protections()
        .await
        .unwrap()
        .iter()
        .any(|protection| protection.key == verify_key));
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
