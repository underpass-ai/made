use std::fs;
use std::sync::Arc;

use made_adapters::artifacts::{ArtifactBackupService, LocalArtifactStore};
use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactPageLimit, ArtifactRetentionActor,
    ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort, BeginArtifactUpload,
    PutArtifactChunk, RestoreProtectionKey, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactRef, ArtifactSizeBytes,
    DurationMs, IdempotencyKey, LeaseOwnerId, StepLease,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
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

async fn upload(
    store: &LocalArtifactStore,
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

fn lease(key: &str) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("restore-test-maintenance").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        OffsetDateTime::UNIX_EPOCH,
        DurationMs::from_millis(259_200_000),
    )
    .unwrap()
}

async fn tombstone(store: &LocalArtifactStore, artifact: &ArtifactRef, policy: &str) {
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

#[tokio::test]
async fn completed_restore_retry_releases_only_restore_pin_and_gc_respects_receipt_pin() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let referenced = upload(&source, b"receipt referenced", "source-referenced", None).await;
    let unreferenced = upload(&source, b"safe to collect", "source-unreferenced", None).await;

    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();

    let restored = Arc::new(LocalArtifactStore::open(directory.path().join("restored")).unwrap());
    let service = ArtifactBackupService::new(restored.clone());
    service.restore_from(&backup).await.unwrap();
    let receipt_key = ArtifactIdempotencyKey::new("receipt:restore-referenced").unwrap();
    restored
        .protect_references(receipt_key.clone(), vec![referenced.artifact_id().clone()])
        .await
        .unwrap();

    // A completed restore is safe to replay, but the released restore pin is
    // never recreated and the durable receipt pin remains live.
    service.restore_from(&backup).await.unwrap();
    let protections = restored.active_protections().await.unwrap();
    assert_eq!(protections.len(), 1);
    assert_eq!(protections[0].key, receipt_key);
    tombstone(&restored, &referenced, "restore-referenced").await;
    tombstone(&restored, &unreferenced, "restore-unreferenced").await;

    let plan = restored
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("restore-gc"),
        )
        .await
        .unwrap();
    assert_eq!(plan.candidates.len(), 1);
    assert_eq!(plan.candidates[0].digest, *unreferenced.digest());
    let report = restored
        .apply_gc(&plan, OffsetDateTime::UNIX_EPOCH + time::Duration::days(2))
        .await
        .unwrap();
    assert_eq!(report.deleted, vec![unreferenced.digest().clone()]);
    assert_eq!(report.reclaimed_bytes, b"safe to collect".len() as u64);
    assert!(restored
        .backup_content_available(referenced.artifact_id())
        .await
        .unwrap());
    assert!(!restored
        .backup_content_available(unreferenced.artifact_id())
        .await
        .unwrap());
}

#[tokio::test]
async fn finish_restore_rechecks_target_and_keeps_pin_until_repaired() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let artifact = upload(&source, b"finish must verify", "finish-source", None).await;
    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();

    let target_root = directory.path().join("target");
    let target = Arc::new(LocalArtifactStore::open(&target_root).unwrap());
    let service = ArtifactBackupService::new(target.clone());
    service.restore_from_protected(&backup).await.unwrap();
    let blob = target_root
        .join("blobs")
        .join(artifact.digest().as_str().trim_start_matches("sha256:"));
    fs::write(&blob, b"corrupt after publication").unwrap();

    assert_eq!(
        service.finish_restore(&backup).await,
        Err(ArtifactStoreError::Incomplete {
            expected: b"finish must verify".len() as u64,
            actual: b"corrupt after publication".len() as u64,
        })
    );
    assert_eq!(target.active_protections().await.unwrap().len(), 1);

    let source_blob = fs::read_dir(backup.join("blobs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::copy(source_blob, &blob).unwrap();
    service.finish_restore(&backup).await.unwrap();
    assert!(target.active_protections().await.unwrap().is_empty());
}

#[tokio::test]
async fn protected_restore_rehydrates_receipt_before_finish_and_fences_stale_gc() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let referenced = upload(&source, b"composed receipt", "composed-referenced", None).await;
    let unreferenced = upload(&source, b"composed collect", "composed-unreferenced", None).await;
    tombstone(&source, &referenced, "composed-referenced").await;
    tombstone(&source, &unreferenced, "composed-unreferenced").await;
    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();

    let target = Arc::new(LocalArtifactStore::open(directory.path().join("target")).unwrap());
    upload(
        &target,
        b"composed receipt",
        "target-referenced",
        Some(&referenced),
    )
    .await;
    upload(
        &target,
        b"composed collect",
        "target-unreferenced",
        Some(&unreferenced),
    )
    .await;
    tombstone(&target, &referenced, "composed-referenced").await;
    tombstone(&target, &unreferenced, "composed-unreferenced").await;

    let stale_plan = target
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("stale-restore-gc"),
        )
        .await
        .unwrap();
    assert_eq!(stale_plan.candidates.len(), 2);

    let service = ArtifactBackupService::new(target.clone());
    service.restore_from_protected(&backup).await.unwrap();
    assert_eq!(target.active_protections().await.unwrap().len(), 1);

    // The stale plan crosses only the temporary restore fence here: the
    // durable receipt has deliberately not been recreated yet.
    assert_eq!(
        target
            .apply_gc(
                &stale_plan,
                OffsetDateTime::UNIX_EPOCH + time::Duration::days(2),
            )
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    assert!(target
        .backup_content_available(unreferenced.artifact_id())
        .await
        .unwrap());

    let receipt_key = ArtifactIdempotencyKey::new("receipt:composed-referenced").unwrap();
    target
        .protect_references(receipt_key.clone(), vec![referenced.artifact_id().clone()])
        .await
        .unwrap();
    assert_eq!(target.active_protections().await.unwrap().len(), 2);

    service.finish_restore(&backup).await.unwrap();
    let protections = target.active_protections().await.unwrap();
    assert_eq!(protections.len(), 1);
    assert_eq!(protections[0].key, receipt_key);
    assert_eq!(
        target
            .apply_gc(
                &stale_plan,
                OffsetDateTime::UNIX_EPOCH + time::Duration::days(2),
            )
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let current_plan = target
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("current-restore-gc"),
        )
        .await
        .unwrap();
    let collected = target
        .apply_gc(
            &current_plan,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(2),
        )
        .await
        .unwrap();
    assert_eq!(collected.deleted, vec![unreferenced.digest().clone()]);
    assert!(target
        .backup_content_available(referenced.artifact_id())
        .await
        .unwrap());
}

#[tokio::test]
async fn interrupted_restore_keeps_pin_and_retries_after_target_conflict_is_fixed() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let artifact = upload(&source, b"retryable restore", "source-retry", None).await;
    tombstone(&source, &artifact, "retryable-restore").await;
    let source_record = source.get(artifact.artifact_id()).await.unwrap();

    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();

    let restored_root = directory.path().join("restored");
    let restored = Arc::new(LocalArtifactStore::open(&restored_root).unwrap());
    // The same immutable artifact exists, but its tombstone is not present.
    // The restore pin must be durable before this validation failure.
    upload(
        &restored,
        b"retryable restore",
        "target-conflict",
        Some(&artifact),
    )
    .await;
    let service = ArtifactBackupService::new(restored.clone());
    assert_eq!(
        service.restore_from(&backup).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let protections = restored.active_protections().await.unwrap();
    assert_eq!(protections.len(), 1);
    assert!(RestoreProtectionKey::try_from(protections[0].key.clone()).is_ok());

    // Model a process restart after the restore pin was made durable.
    drop(service);
    drop(restored);
    let restored = Arc::new(LocalArtifactStore::open(&restored_root).unwrap());
    let restarted = ArtifactBackupService::new(restored.clone());

    restored
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: source_record.tombstone.as_ref().unwrap().actor.clone(),
            policy: source_record.tombstone.as_ref().unwrap().policy.clone(),
            retired_at: source_record.tombstone.as_ref().unwrap().retired_at,
        })
        .await
        .unwrap();
    restarted.restore_from(&backup).await.unwrap();
    assert!(restored.active_protections().await.unwrap().is_empty());

    // A retry after release verifies the restored state without recreating
    // the temporary restore protection.
    restarted.restore_from(&backup).await.unwrap();
    assert!(restored.active_protections().await.unwrap().is_empty());
}

#[tokio::test]
async fn restore_release_key_rejects_receipt_namespace_at_the_boundary() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let artifact = upload(&store, b"receipt cannot be released", "receipt", None).await;
    let receipt_key = ArtifactIdempotencyKey::new("receipt:protected").unwrap();
    store
        .protect_references(receipt_key.clone(), vec![artifact.artifact_id().clone()])
        .await
        .unwrap();

    assert!(RestoreProtectionKey::try_from(receipt_key).is_err());
    assert_eq!(store.active_protections().await.unwrap().len(), 1);
    assert_eq!(
        store
            .list(None, ArtifactPageLimit::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}
