use std::fs;
use std::process::Command;
use std::sync::Arc;

use made_adapters::artifacts::{ArtifactBackupService, LocalArtifactStore};
use made_app::artifacts::ArtifactService;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
    ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::OffsetDateTime;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

fn begin(bytes: &[u8], key: &str) -> BeginArtifactUpload {
    BeginArtifactUpload {
        requested_artifact_id: None,
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
) -> made_core::value_objects::ArtifactRef {
    let status = store.begin_upload(begin(bytes, key)).await.unwrap();
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

#[tokio::test]
async fn independent_handles_serialize_the_same_idempotent_begin() {
    let directory = TempDir::new().unwrap();
    let first = LocalArtifactStore::open(directory.path()).unwrap();
    let second = LocalArtifactStore::open(directory.path()).unwrap();
    let bytes = b"same request";
    let (left, right) = tokio::join!(
        first.begin_upload(begin(bytes, "two-handles")),
        second.begin_upload(begin(bytes, "two-handles"))
    );
    assert_eq!(left.unwrap().upload_id, right.unwrap().upload_id);
    assert_eq!(
        fs::read_dir(directory.path().join("idempotency"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn independent_processes_share_one_idempotent_upload() {
    let directory = TempDir::new().unwrap();
    let children = [(), ()].map(|()| {
        Command::new(env!("CARGO_BIN_EXE_artifact_store_writer"))
            .arg(directory.path())
            .spawn()
            .expect("artifact writer spawns")
    });
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    assert_eq!(
        fs::read_dir(directory.path().join("artifacts"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read_dir(directory.path().join("blobs"))
            .unwrap()
            .count(),
        1
    );
}

#[tokio::test]
async fn upload_resumes_after_restart_and_repeated_chunks_are_idempotent() {
    let directory = TempDir::new().unwrap();
    let bytes = b"durable artifact across restart";
    let first = LocalArtifactStore::open(directory.path()).unwrap();
    let status = first
        .begin_upload(begin(bytes, "restart-key"))
        .await
        .unwrap();
    let split = 10;
    first
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes[..split].to_vec(),
            chunk_digest: digest(&bytes[..split]),
        })
        .await
        .unwrap();
    drop(first);

    let reopened = LocalArtifactStore::open(directory.path()).unwrap();
    let resumed = reopened
        .begin_upload(begin(bytes, "restart-key"))
        .await
        .unwrap();
    assert_eq!(resumed.upload_id, status.upload_id);
    assert_eq!(resumed.next_offset.get(), split as u64);
    let repeated = reopened
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes[..split].to_vec(),
            chunk_digest: digest(&bytes[..split]),
        })
        .await
        .unwrap();
    assert_eq!(repeated.next_offset.get(), split as u64);
    assert!(matches!(
        reopened
            .put_chunk(PutArtifactChunk {
                upload_id: status.upload_id.clone(),
                offset: ArtifactByteOffset::ZERO,
                bytes: b"different!".to_vec(),
                chunk_digest: digest(b"different!"),
            })
            .await,
        Err(ArtifactStoreError::UnexpectedOffset { .. })
    ));
    reopened
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::new(split as u64),
            bytes: bytes[split..].to_vec(),
            chunk_digest: digest(&bytes[split..]),
        })
        .await
        .unwrap();
    let artifact = reopened.commit_upload(&status.upload_id).await.unwrap();
    assert_eq!(
        reopened.commit_upload(&status.upload_id).await.unwrap(),
        artifact
    );
    let chunk = reopened
        .read_chunk(ReadArtifactChunk {
            artifact_id: artifact.artifact_id().clone(),
            offset: ArtifactByteOffset::ZERO,
            max_bytes: ArtifactChunkLimit::new(1024).unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(chunk.bytes, bytes);
    assert!(chunk.is_complete());
}

#[tokio::test]
async fn incomplete_wrong_digest_and_oversized_chunks_are_refused() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let bytes = b"complete me";
    let status = store
        .begin_upload(begin(bytes, "incomplete"))
        .await
        .unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes[..3].to_vec(),
            chunk_digest: digest(&bytes[..3]),
        })
        .await
        .unwrap();
    assert_eq!(
        store.commit_upload(&status.upload_id).await,
        Err(ArtifactStoreError::Incomplete {
            expected: bytes.len() as u64,
            actual: 3,
        })
    );
    assert_eq!(
        store
            .put_chunk(PutArtifactChunk {
                upload_id: status.upload_id.clone(),
                offset: ArtifactByteOffset::new(3),
                bytes: b"bad".to_vec(),
                chunk_digest: digest(b"not bad"),
            })
            .await,
        Err(ArtifactStoreError::ChunkDigestMismatch)
    );
    let oversized = vec![7; ARTIFACT_MAX_CHUNK_BYTES as usize + 1];
    assert!(matches!(
        store
            .put_chunk(PutArtifactChunk {
                upload_id: status.upload_id,
                offset: ArtifactByteOffset::new(3),
                chunk_digest: digest(&oversized),
                bytes: oversized,
            })
            .await,
        Err(ArtifactStoreError::ChunkTooLarge { .. })
    ));
}

#[tokio::test]
async fn equal_content_is_deduplicated_while_artifact_identity_stays_distinct() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let first = upload(&store, b"same", "dedup-1").await;
    let second = upload(&store, b"same", "dedup-2").await;
    assert_ne!(first.artifact_id(), second.artifact_id());
    assert_eq!(first.digest(), second.digest());
    assert_eq!(
        fs::read_dir(directory.path().join("blobs"))
            .unwrap()
            .count(),
        1
    );
}

#[tokio::test]
async fn generated_report_is_saved_through_the_public_application_facade() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let service = ArtifactService::new(store.clone());
    let report = service
        .save_generated_report(
            b"# Ceremony report\nverified\n",
            ArtifactMediaType::new("text/markdown").unwrap(),
            OffsetDateTime::UNIX_EPOCH,
            ArtifactIdempotencyKey::new("report:ceremony-1").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        report.provenance().source_kind(),
        made_core::value_objects::ArtifactSourceKind::GeneratedReport
    );
    assert_eq!(
        store.get(report.artifact_id()).await.unwrap().artifact,
        report
    );
}

#[tokio::test]
async fn backup_restore_preserves_tombstone_and_detects_tamper_or_missing_content() {
    let directory = TempDir::new().unwrap();
    let source_root = directory.path().join("source");
    let source = Arc::new(LocalArtifactStore::open(&source_root).unwrap());
    let artifact = upload(&source, b"backup body", "backup-source").await;
    let tombstone = source
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:test").unwrap(),
            policy: ArtifactRetentionPolicy::new("test-expiry").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    assert_eq!(
        source
            .read_chunk(ReadArtifactChunk {
                artifact_id: artifact.artifact_id().clone(),
                offset: ArtifactByteOffset::ZERO,
                max_bytes: ArtifactChunkLimit::new(10).unwrap(),
            })
            .await,
        Err(ArtifactStoreError::Tombstoned)
    );

    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();
    let restored = Arc::new(LocalArtifactStore::open(directory.path().join("restored")).unwrap());
    ArtifactBackupService::new(restored.clone())
        .restore_from(&backup)
        .await
        .unwrap();
    let record = restored.get(artifact.artifact_id()).await.unwrap();
    assert_eq!(record.artifact, artifact);
    assert_eq!(record.tombstone, Some(tombstone));

    let blob = fs::read_dir(backup.join("blobs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&blob, b"tampered").unwrap();
    let empty = Arc::new(LocalArtifactStore::open(directory.path().join("empty")).unwrap());
    assert_eq!(
        ArtifactBackupService::new(empty.clone())
            .restore_from(&backup)
            .await,
        Err(ArtifactStoreError::InvalidBackup)
    );
    assert!(empty
        .list(None, ArtifactPageLimit::default())
        .await
        .unwrap()
        .items
        .is_empty());
    fs::remove_file(blob).unwrap();
    let another = Arc::new(LocalArtifactStore::open(directory.path().join("another")).unwrap());
    assert_eq!(
        ArtifactBackupService::new(another)
            .restore_from(&backup)
            .await,
        Err(ArtifactStoreError::InvalidBackup)
    );
}
