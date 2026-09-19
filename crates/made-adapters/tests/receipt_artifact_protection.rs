use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_app::artifacts::ArtifactService;
use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactRetentionActor, ArtifactRetentionPolicy,
    ArtifactStorePort, BeginArtifactUpload, PutArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactRef, ArtifactSizeBytes,
    ArtifactSourceKind, DurationMs, ExecutionConnectorId, ExecutionOperationId, ExecutionReceipt,
    ExecutionReceiptId, ExecutionRecoveryCapability, ExecutionRequestDigest, IdempotencyKey,
    LeaseOwnerId, StepClaimFence, StepLease, StepOutput, StepResult,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::OffsetDateTime;

fn receipt(artifact: ArtifactRef) -> ExecutionReceipt {
    ExecutionReceipt::new(
        ExecutionOperationId::new("1".repeat(64)).unwrap(),
        ExecutionRequestDigest::new("2".repeat(64)).unwrap(),
        StepClaimFence::new("3".repeat(64)).unwrap(),
        ExecutionConnectorId::new("fixture").unwrap(),
        None,
        ExecutionRecoveryCapability::IdempotentByOperationId,
        ArtifactSourceKind::NoOp,
        StepResult::completed(StepOutput::empty()).unwrap(),
        vec![artifact],
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

async fn upload(store: &LocalArtifactStore) -> ArtifactRef {
    let body = b"receipt evidence must survive retirement";
    let digest = ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(body))).unwrap();
    let operation = ExecutionOperationId::new("1".repeat(64)).unwrap();
    let request = BeginArtifactUpload {
        requested_artifact_id: None,
        expected_digest: digest.clone(),
        size_bytes: ArtifactSizeBytes::new(body.len() as u64),
        media_type: ArtifactMediaType::new("text/plain").unwrap(),
        provenance: ArtifactProvenance::execution(
            ArtifactSourceKind::NoOp,
            ExecutionReceiptId::for_operation(&operation),
            operation,
            StepClaimFence::new("3".repeat(64)).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap(),
        idempotency_key: ArtifactIdempotencyKey::new("receipt-upload").unwrap(),
    };
    let upload = store.begin_upload(request).await.unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest,
            bytes: body.to_vec(),
        })
        .await
        .unwrap();
    store.commit_upload(&upload.upload_id).await.unwrap()
}

#[tokio::test]
async fn receipt_protection_survives_reopen_and_blocks_an_earlier_gc_plan() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let artifact = upload(&store).await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("test").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let lease = StepLease::acquire(
        LeaseOwnerId::new("gc").unwrap(),
        IdempotencyKey::new("gc").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
        DurationMs::from_millis(259_200_000),
    )
    .unwrap();
    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(1);
    let stale_plan = store.plan_gc(cutoff, lease.clone()).await.unwrap();
    assert_eq!(stale_plan.candidates.len(), 1);
    let receipt = receipt(artifact);
    let service = ArtifactService::new(store);
    service.protect_execution_receipt(&receipt).await.unwrap();
    service.protect_execution_receipt(&receipt).await.unwrap();
    let reopened = LocalArtifactStore::open(directory.path()).unwrap();
    assert!(reopened.apply_gc(&stale_plan, cutoff).await.is_err());
    assert!(reopened
        .plan_gc(cutoff, lease)
        .await
        .unwrap()
        .candidates
        .is_empty());
}

#[tokio::test]
async fn receipt_with_changed_artifact_metadata_is_not_accepted() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let artifact = upload(&store).await;
    let changed = ArtifactRef::new(
        artifact.artifact_id().clone(),
        artifact.digest().clone(),
        artifact.size_bytes(),
        ArtifactMediaType::new("application/json").unwrap(),
        artifact.provenance().clone(),
    );
    assert!(ArtifactService::new(store)
        .protect_execution_receipt(&receipt(changed))
        .await
        .is_err());
}

#[tokio::test]
async fn receipt_cannot_accept_retired_metadata_after_bytes_were_collected() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let artifact = upload(&store).await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("test").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let lease = StepLease::acquire(
        LeaseOwnerId::new("gc").unwrap(),
        IdempotencyKey::new("gc").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
        DurationMs::from_millis(259_200_000),
    )
    .unwrap();
    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(1);
    let plan = store.plan_gc(cutoff, lease).await.unwrap();
    store.apply_gc(&plan, cutoff).await.unwrap();
    assert!(!store
        .backup_content_available(artifact.artifact_id())
        .await
        .unwrap());
    assert!(ArtifactService::new(store)
        .protect_execution_receipt(&receipt(artifact))
        .await
        .is_err());
}
