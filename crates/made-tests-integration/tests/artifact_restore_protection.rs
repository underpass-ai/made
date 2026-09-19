#![cfg(feature = "container-tests")]

use std::sync::Arc;
use std::time::Duration;

use made_adapters::artifacts::ArtifactBackupService;
use made_adapters::postgres::{PostgresArtifactStore, PostgresConfig, PostgresPool};
use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactRetentionActor, ArtifactRetentionPolicy,
    ArtifactStorePort, BeginArtifactUpload, PutArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactRef, ArtifactSizeBytes,
    DurationMs, IdempotencyKey, LeaseOwnerId, StepLease,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use time::OffsetDateTime;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

fn begin(bytes: &[u8], key: &str, artifact_id: Option<&ArtifactRef>) -> BeginArtifactUpload {
    BeginArtifactUpload {
        requested_artifact_id: artifact_id.map(|artifact| artifact.artifact_id().clone()),
        expected_digest: digest(bytes),
        size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
        media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
        provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
        idempotency_key: ArtifactIdempotencyKey::new(key).unwrap(),
    }
}

async fn postgres() -> (
    PostgresPool,
    PostgresConfig,
    testcontainers::ContainerAsync<GenericImage>,
) {
    let container = GenericImage::new("postgres", "16-alpine")
        .with_exposed_port(5432_u16.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "made")
        .with_env_var("POSTGRES_PASSWORD", "made")
        .with_env_var("POSTGRES_DB", "made")
        .start()
        .await
        .unwrap();
    let port = container.get_host_port_ipv4(5432_u16.tcp()).await.unwrap();
    let url = format!("postgres://made:made@127.0.0.1:{port}/made");
    let mut config = PostgresConfig::from_url(url);
    config.acquire_timeout = Duration::from_secs(10);
    let mut last = None;
    for _ in 0..20 {
        match PostgresPool::connect(&config).await {
            Ok(pool) => {
                pool.run_migrations().await.unwrap();
                return (pool, config, container);
            }
            Err(error) => {
                last = Some(error);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    panic!("postgres did not start: {last:?}");
}

async fn upload(
    store: &PostgresArtifactStore,
    bytes: &[u8],
    key: &str,
    artifact_id: Option<&ArtifactRef>,
) -> ArtifactRef {
    let status = store
        .begin_upload(begin(bytes, key, artifact_id))
        .await
        .unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes.to_vec(),
            chunk_digest: digest(bytes),
        })
        .await
        .unwrap();
    store.commit_upload(&status.upload_id).await.unwrap()
}

async fn tombstone(store: &PostgresArtifactStore, artifact: &ArtifactRef, policy: &str) {
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:restore-test").unwrap(),
            policy: ArtifactRetentionPolicy::new(policy).unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
}

fn lease(key: &str) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("restore-test-maintenance").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        OffsetDateTime::now_utc(),
        DurationMs::from_millis(259_200_000),
    )
    .unwrap()
}

#[tokio::test]
async fn postgres_restore_retry_releases_only_restore_pin_and_gc_preserves_receipt() {
    let (source_pool, _source_config, _source_container) = postgres().await;
    let (target_pool, target_config, _target_container) = postgres().await;
    let source = Arc::new(PostgresArtifactStore::new(source_pool));
    let target = Arc::new(PostgresArtifactStore::new(target_pool));
    let referenced = upload(&source, b"postgres receipt", "source-referenced", None).await;
    let unreferenced = upload(&source, b"postgres collect", "source-unreferenced", None).await;
    tombstone(&source, &referenced, "postgres-referenced").await;
    tombstone(&source, &unreferenced, "postgres-unreferenced").await;

    let directory = tempdir().unwrap();
    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();

    upload(
        &target,
        b"postgres receipt",
        "target-referenced",
        Some(&referenced),
    )
    .await;
    upload(
        &target,
        b"postgres collect",
        "target-unreferenced",
        Some(&unreferenced),
    )
    .await;
    tombstone(&target, &referenced, "postgres-referenced").await;
    tombstone(&target, &unreferenced, "postgres-unreferenced").await;
    let stale_plan = target
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("stale-postgres-restore-gc"),
        )
        .await
        .unwrap();
    assert_eq!(stale_plan.candidates.len(), 2);

    let service = ArtifactBackupService::new(target.clone());
    service.restore_from_protected(&backup).await.unwrap();

    // Before receipt rehydration, only the temporary restore protection can
    // invalidate this stale plan.
    assert_eq!(target.active_protections().await.unwrap().len(), 1);
    assert_eq!(
        target
            .apply_gc(&stale_plan, OffsetDateTime::now_utc())
            .await,
        Err(made_core::ports::ArtifactStoreError::IdempotencyConflict)
    );
    assert!(target
        .backup_content_available(unreferenced.artifact_id())
        .await
        .unwrap());

    // Rebuild every process-local handle, then resume against the persisted
    // restore fence before publishing the durable receipt protection.
    drop(service);
    drop(target);
    let restarted_pool = PostgresPool::connect(&target_config).await.unwrap();
    restarted_pool.run_migrations().await.unwrap();
    let target = Arc::new(PostgresArtifactStore::new(restarted_pool));
    let service = ArtifactBackupService::new(target.clone());
    service.restore_from_protected(&backup).await.unwrap();

    let receipt_key = ArtifactIdempotencyKey::new("receipt:postgres-referenced").unwrap();
    target
        .protect_references(receipt_key.clone(), vec![referenced.artifact_id().clone()])
        .await
        .unwrap();
    assert_eq!(target.active_protections().await.unwrap().len(), 2);
    service.finish_restore(&backup).await.unwrap();
    let protections = target.active_protections().await.unwrap();
    assert_eq!(protections.len(), 1);
    assert_eq!(protections[0].key, receipt_key);

    // A replay after release verifies the durable target state and leaves the
    // temporary restore pin released.
    service.restore_from(&backup).await.unwrap();
    let protections = target.active_protections().await.unwrap();
    assert_eq!(protections.len(), 1);
    assert_eq!(protections[0].key, receipt_key);

    assert_eq!(
        target
            .apply_gc(&stale_plan, OffsetDateTime::now_utc())
            .await,
        Err(made_core::ports::ArtifactStoreError::IdempotencyConflict)
    );
    let plan = target
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("postgres-restore-gc"),
        )
        .await
        .unwrap();
    assert_eq!(plan.candidates.len(), 1);
    assert_eq!(plan.candidates[0].digest, *unreferenced.digest());
    let report = target
        .apply_gc(&plan, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert_eq!(report.deleted, vec![unreferenced.digest().clone()]);
    assert!(target
        .backup_content_available(referenced.artifact_id())
        .await
        .unwrap());
}
