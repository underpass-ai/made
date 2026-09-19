#![cfg(feature = "container-tests")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use made_adapters::artifacts::ArtifactGcExclusionReason;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresBackupService, PostgresConfig, PostgresPool,
};
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
    DurationMs, IdempotencyKey, LeaseOwnerId, StepLease,
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

#[cfg(unix)]
#[tokio::test]
async fn postgres_backup_service_verifies_archive_and_restores_only_to_empty_database() {
    let (pool, url, container) = start_with_url().await;
    let store = PostgresArtifactStore::new(pool);
    let artifact = upload(&store, b"service archive boundary").await;
    let scratch = tempfile::TempDir::new().unwrap();
    let dump = scratch.path().join("pg_dump-wrapper");
    let restore = scratch.path().join("pg_restore-wrapper");
    std::fs::write(
        &dump,
        format!(
            "#!/bin/sh\nset -eu\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = '--file' ]; then shift; out=$1; fi\n  shift || true\ndone\ndocker exec {} pg_dump -U made -Fc -f /tmp/service.dump made\ndocker cp {}:/tmp/service.dump \"$out\" >/dev/null\n",
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

    let service =
        PostgresBackupService::new(store, url.clone()).with_client_programs(&dump, &restore);
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
    let writer = tokio::spawn(async move {
        while !writer_stop.load(Ordering::Acquire) {
            sqlx::query("INSERT INTO service_writer DEFAULT VALUES")
                .execute(&writer_pool)
                .await
                .unwrap();
        }
    });
    let backup_started = std::time::Instant::now();
    let manifest = service.backup_to(&backup, key).await.unwrap();
    let backup_duration = backup_started.elapsed();
    stop.store(true, Ordering::Release);
    writer.await.unwrap();
    let source_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_writer")
        .fetch_one(&raw)
        .await
        .unwrap();
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
    service.restore_to(&backup, &target_url).await.unwrap();
    let restore_duration = restore_started.elapsed();
    let target = PostgresArtifactStore::new(
        PostgresPool::connect(&PostgresConfig::from_url(target_url.clone()))
            .await
            .unwrap(),
    );
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
    assert_eq!(
        target
            .read_chunk_for_backup(ReadArtifactChunk {
                artifact_id: artifact.artifact_id().clone(),
                offset: ArtifactByteOffset::ZERO,
                max_bytes: ArtifactChunkLimit::default(),
            })
            .await
            .unwrap()
            .bytes,
        b"service archive boundary"
    );
    assert!(!manifest.snapshot_id.is_empty());
    assert!(!manifest.transaction_snapshot.is_empty());
    assert!(target
        .backup_content_available(artifact.artifact_id())
        .await
        .unwrap());
    assert!(target.active_protections().await.unwrap().is_empty());
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
    assert!(restored_rows > 0 && restored_rows <= source_rows);
    eprintln!(
        "postgres backup metrics: backup_ms={} restore_ms={} source_rows={} restored_rows={} rpo_missing_rows={} rpo_lag_ms={}",
        backup_duration.as_millis(),
        restore_duration.as_millis(),
        source_rows,
        restored_rows,
        source_rows - restored_rows,
        source_last_ms - restored_last_ms
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
