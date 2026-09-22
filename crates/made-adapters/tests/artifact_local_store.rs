use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use made_adapters::artifacts::{
    ArtifactBackupEntry, ArtifactBackupService, ArtifactGcExclusionReason,
    ArtifactRetentionService, LocalArtifactStore,
};
#[cfg(feature = "sqlite")]
use made_adapters::artifacts::{SqliteArtifactBackupService, SqliteBackupService};
use made_app::artifacts::ArtifactService;
use made_app::services::AuthorizationOperationScope;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
    ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationEvidence, AuthorizedOperation,
    DurationMs, IdempotencyKey, LeaseOwnerId, PrincipalId, PrincipalKind, StepLease,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::OffsetDateTime;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

fn begin(bytes: &[u8], key: &str) -> BeginArtifactUpload {
    begin_at(bytes, key, OffsetDateTime::UNIX_EPOCH)
}

fn begin_at(bytes: &[u8], key: &str, observed_at: OffsetDateTime) -> BeginArtifactUpload {
    BeginArtifactUpload {
        requested_artifact_id: None,
        expected_digest: digest(bytes),
        size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
        media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
        provenance: ArtifactProvenance::generated_report(observed_at),
        idempotency_key: ArtifactIdempotencyKey::new(key).unwrap(),
    }
}

fn lease(key: &str, ttl_ms: u64) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("artifact-maintenance").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        OffsetDateTime::UNIX_EPOCH,
        DurationMs::from_millis(ttl_ms),
    )
    .unwrap()
}

fn authorized_operation(id: &str, action: &str, decision: char) -> AuthorizedOperation {
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap();
    let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
        "decision_id": decision.to_string().repeat(64),
        "request_id": format!("request-{id}-{action}"),
        "principal_id": id,
        "action": action,
        "scope": { "kind": "global" },
        "target_digest": "b".repeat(64),
        "policy_version": 1,
        "admitted_at": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T12:01:00Z"
    }))
    .unwrap();
    AuthorizedOperation::new(principal, evidence).unwrap()
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

async fn upload_at(
    store: &LocalArtifactStore,
    bytes: &[u8],
    key: &str,
    observed_at: OffsetDateTime,
) -> made_core::value_objects::ArtifactRef {
    let status = store
        .begin_upload(begin_at(bytes, key, observed_at))
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
async fn public_commit_and_tombstone_preserve_their_authorization_after_reopen() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let service = ArtifactService::new(store.clone());
    let bytes = b"authorized artifact";
    let upload = service
        .begin_upload(begin(bytes, "authorized"))
        .await
        .unwrap();
    service
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes.to_vec(),
            chunk_digest: digest(bytes),
        })
        .await
        .unwrap();
    let artifact = AuthorizationOperationScope::run(
        authorized_operation("artifact-writer", "commit_artifact_upload", 'a'),
        service.commit_upload(&upload.upload_id),
    )
    .await
    .unwrap();
    let retirement = TombstoneArtifact {
        artifact_id: artifact.artifact_id().clone(),
        actor: ArtifactRetentionActor::new("host:retention").unwrap(),
        policy: ArtifactRetentionPolicy::new("incident-closed").unwrap(),
        retired_at: OffsetDateTime::UNIX_EPOCH,
    };
    let tombstone = AuthorizationOperationScope::run(
        authorized_operation("retention-host", "tombstone_artifact", 'c'),
        service.tombstone(retirement.clone()),
    )
    .await
    .unwrap();
    let retried = AuthorizationOperationScope::run(
        authorized_operation("another-retention-host", "tombstone_artifact", 'd'),
        service.tombstone(retirement),
    )
    .await
    .unwrap();
    assert_eq!(retried, tombstone, "the first evidence stays immutable");
    drop(service);
    drop(store);

    let record = LocalArtifactStore::open(directory.path())
        .unwrap()
        .get(artifact.artifact_id())
        .await
        .unwrap();
    assert_eq!(
        record
            .authorization
            .as_ref()
            .unwrap()
            .principal_id()
            .as_str(),
        "artifact-writer"
    );
    assert_eq!(
        tombstone
            .authorization
            .as_ref()
            .unwrap()
            .principal_id()
            .as_str(),
        "retention-host"
    );
    assert_eq!(record.tombstone, Some(tombstone));
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

#[tokio::test]
async fn backup_rejects_corrupt_source_before_publishing_and_preserves_storage_failures() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    fs::create_dir_all(&scratch).unwrap();
    for prepared in [false, true] {
        for replacement in [
            Some(b"wrong bytes".as_slice()),
            Some(b"short".as_slice()),
            None,
        ] {
            let directory = TempDir::new_in(&scratch).unwrap();
            let source_root = directory.path().join("source");
            let source = Arc::new(LocalArtifactStore::open(&source_root).unwrap());
            let artifact = upload(&source, b"backup body", "backup-corrupt-source").await;
            let service = ArtifactBackupService::new(source.clone());
            let destination = directory.path().join("backup");
            if prepared {
                service.prepare(&destination).await.unwrap();
            }
            let blob = source_root
                .join("blobs")
                .join(artifact.digest().as_str().trim_start_matches("sha256:"));
            let expected = if let Some(bytes) = replacement {
                fs::write(&blob, bytes).unwrap();
                "artifact backup is corrupt or incomplete".to_owned()
            } else {
                fs::remove_file(&blob).unwrap();
                "artifact storage is unavailable: local artifact operation: No such file or directory (os error 2)".to_owned()
            };
            assert_eq!(
                service.plan().await.unwrap_err().to_string(),
                expected.clone(),
                "missing blob must surface as storage-unavailable naming its phase"
            );
            assert_eq!(
                service
                    .backup_to(&destination)
                    .await
                    .unwrap_err()
                    .to_string(),
                expected.clone()
            );
            if prepared {
                let manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(destination.join("manifest.json")).unwrap())
                        .unwrap();
                assert_eq!(manifest["complete"], false);
                assert!(matches!(
                    ArtifactBackupService::<LocalArtifactStore>::inspect_manifest(&destination),
                    Err(ArtifactStoreError::InvalidBackup)
                ));
            } else {
                assert!(
                    !destination.exists(),
                    "a failed backup must not be published"
                );
                let staging = directory.path().join(".backup-in-progress");
                assert!(!staging.join("manifest.json").exists());
            }
            assert_eq!(
                source.active_protections().await.unwrap().len(),
                usize::from(prepared),
                "a failed backup must preserve any existing durable protection"
            );
        }
    }
}

#[tokio::test]
async fn backup_plan_and_manifest_resume_deterministically_and_repeat_idempotently() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let first = upload(&source, b"first backup body", "backup-plan-1").await;
    let second = upload(&source, b"second backup body", "backup-plan-2").await;
    let backup = directory.path().join("backup");
    let service = ArtifactBackupService::new(source);

    let plan = service.prepare(&backup).await.unwrap();
    assert_eq!(plan.entries.len(), 2);
    let pending: serde_json::Value =
        serde_json::from_slice(&fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(pending["complete"], false);

    service.backup_to(&backup).await.unwrap();
    let manifest = ArtifactBackupService::<LocalArtifactStore>::inspect_manifest(&backup).unwrap();
    assert!(manifest.is_complete());
    let mut expected_ids = vec![first.artifact_id().clone(), second.artifact_id().clone()];
    expected_ids.sort();
    assert_eq!(
        manifest
            .plan
            .entries
            .iter()
            .map(ArtifactBackupEntry::artifact_id)
            .cloned()
            .collect::<Vec<_>>(),
        expected_ids
    );
    let bytes_before = fs::read(backup.join("manifest.json")).unwrap();
    service.backup_to(&backup).await.unwrap();
    assert_eq!(
        fs::read(backup.join("manifest.json")).unwrap(),
        bytes_before
    );
}

#[tokio::test]
async fn prepared_backup_pins_content_against_tombstone_and_gc() {
    let directory = TempDir::new().unwrap();
    let source_root = directory.path().join("source");
    let source = Arc::new(LocalArtifactStore::open(&source_root).unwrap());
    let body = b"prepared backup must survive garbage collection";
    let artifact = upload(&source, body, "backup-gc-race").await;
    let backup = directory.path().join("backup");
    let service = ArtifactBackupService::new(source.clone());

    service.prepare(&backup).await.unwrap();
    source
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("prepared-backup").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();

    let plan = source
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("prepared-backup-gc", 259_200_000),
        )
        .await
        .unwrap();
    assert!(plan.candidates.is_empty());

    service.backup_to(&backup).await.unwrap();
    let manifest = ArtifactBackupService::<LocalArtifactStore>::inspect_manifest(&backup).unwrap();
    assert!(manifest.is_complete());
    assert_eq!(manifest.plan.entries.len(), 1);
    assert_eq!(
        manifest.plan.entries[0].artifact_id(),
        artifact.artifact_id()
    );

    let released_plan = source
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("completed-backup-gc", 259_200_000),
        )
        .await
        .unwrap();
    assert_eq!(released_plan.candidates.len(), 1);
    let report = source
        .apply_gc(
            &released_plan,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(2),
        )
        .await
        .unwrap();
    assert_eq!(report.deleted, vec![artifact.digest().clone()]);
    assert_eq!(report.reclaimed_bytes, body.len() as u64);
}

#[tokio::test]
async fn backup_preserves_retired_metadata_when_content_was_already_collected() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let artifact = upload(&source, b"retired before backup", "retired-metadata").await;
    source
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("already-collected").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let gc = source
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("retired-metadata-gc", 259_200_000),
        )
        .await
        .unwrap();
    source
        .apply_gc(&gc, OffsetDateTime::UNIX_EPOCH + time::Duration::days(2))
        .await
        .unwrap();

    let backup = directory.path().join("backup");
    ArtifactBackupService::new(source)
        .backup_to(&backup)
        .await
        .unwrap();
    let manifest = ArtifactBackupService::<LocalArtifactStore>::inspect_manifest(&backup).unwrap();
    assert_eq!(
        manifest.plan.entries[0].content,
        made_adapters::artifacts::ArtifactBackupContent::RetiredMetadataOnly
    );
    assert!(fs::read_dir(backup.join("blobs")).unwrap().next().is_none());

    let restored = Arc::new(LocalArtifactStore::open(directory.path().join("restored")).unwrap());
    ArtifactBackupService::new(restored.clone())
        .restore_from(&backup)
        .await
        .unwrap();
    assert!(restored
        .get(artifact.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_some());
    assert!(restored
        .read_chunk_for_backup(ReadArtifactChunk {
            artifact_id: artifact.artifact_id().clone(),
            offset: ArtifactByteOffset::ZERO,
            max_bytes: ArtifactChunkLimit::DEFAULT,
        })
        .await
        .is_err());
}

#[tokio::test]
async fn prepare_recovers_the_original_selection_after_pin_before_manifest() {
    let directory = TempDir::new().unwrap();
    let source = Arc::new(LocalArtifactStore::open(directory.path().join("source")).unwrap());
    let first = upload(&source, b"first protected body", "pin-before-manifest-1").await;
    let backup = directory.path().join("backup");
    let service = ArtifactBackupService::new(source.clone());

    let original = service.prepare(&backup).await.unwrap();
    fs::remove_file(backup.join("manifest.json")).unwrap();
    fs::remove_file(backup.join("plan.json")).unwrap();
    let second = upload(&source, b"later body", "pin-before-manifest-2").await;

    let recovered = service.prepare(&backup).await.unwrap();
    assert_eq!(recovered, original);
    assert_eq!(recovered.entries.len(), 1);
    assert_eq!(recovered.entries[0].artifact_id(), first.artifact_id());
    assert!(recovered
        .entries
        .iter()
        .all(|entry| entry.artifact_id() != second.artifact_id()));
}

#[tokio::test]
async fn protection_added_after_gc_plan_fences_stale_apply() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let body = b"stale gc plan";
    let artifact = upload(&store, body, "stale-gc-plan").await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("stale-plan").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let plan = store
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("stale-plan-gc", 259_200_000),
        )
        .await
        .unwrap();
    assert_eq!(plan.candidates.len(), 1);

    store
        .protect_references(
            ArtifactIdempotencyKey::new("receipt:stale-plan").unwrap(),
            vec![artifact.artifact_id().clone()],
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .apply_gc(&plan, OffsetDateTime::UNIX_EPOCH + time::Duration::days(2))
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    assert!(directory
        .path()
        .join("blobs")
        .join(artifact.digest().as_str().trim_start_matches("sha256:"))
        .exists());
}

#[tokio::test]
async fn protection_survives_reopen_and_release_is_an_idempotent_tombstone() {
    let directory = TempDir::new().unwrap();
    let first = LocalArtifactStore::open(directory.path()).unwrap();
    let artifact = upload(&first, b"durable protection", "durable-protection").await;
    let key = ArtifactIdempotencyKey::new("backup:durable-protection").unwrap();
    let selected = first
        .protect_references(key.clone(), vec![artifact.artifact_id().clone()])
        .await
        .unwrap();
    drop(first);

    let reopened = LocalArtifactStore::open(directory.path()).unwrap();
    assert_eq!(
        reopened
            .protect_references(key.clone(), vec![artifact.artifact_id().clone()])
            .await
            .unwrap(),
        selected
    );
    reopened.release_snapshot(&key).await.unwrap();
    reopened.release_snapshot(&key).await.unwrap();
    assert_eq!(
        reopened
            .protect_references(key, vec![artifact.artifact_id().clone()])
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
}

#[tokio::test]
async fn database_capture_runs_after_the_artifact_pin_is_durable() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let artifact = upload(&store, b"database snapshot boundary", "database-boundary").await;
    let protections = directory.path().join("protections");

    let snapshot = store
        .protect_snapshot_and_then(
            ArtifactIdempotencyKey::new("backup:database-boundary").unwrap(),
            move || {
                let persisted = fs::read_dir(protections)
                    .map_err(|_| ArtifactStoreError::StorageUnavailable {
                        detail: String::new(),
                    })?
                    .count();
                if persisted != 1 {
                    return Err(ArtifactStoreError::StorageUnavailable {
                        detail: String::new(),
                    });
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].artifact, artifact);
}

#[tokio::test]
async fn failed_database_capture_leaves_a_conservative_artifact_pin() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let artifact = upload(&store, b"failed database capture", "failed-database").await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("failed-database").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();

    assert_eq!(
        store
            .protect_snapshot_and_then(
                ArtifactIdempotencyKey::new("backup:failed-database").unwrap(),
                || Err(ArtifactStoreError::StorageUnavailable {
                    detail: String::new()
                }),
            )
            .await,
        Err(ArtifactStoreError::StorageUnavailable {
            detail: String::new()
        })
    );
    let plan = store
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("failed-database-gc", 259_200_000),
        )
        .await
        .unwrap();
    assert!(plan.candidates.is_empty());
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_backup_is_online_verifiable_resumable_and_restores_in_isolation() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("made.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; \
             CREATE TABLE journal (position INTEGER PRIMARY KEY, payload TEXT NOT NULL); \
             INSERT INTO journal(position, payload) VALUES (1, 'before-snapshot');",
        )
        .unwrap();
    let artifacts = LocalArtifactStore::open(directory.path().join("artifacts")).unwrap();
    let artifact = upload(&artifacts, b"sqlite referenced blob", "sqlite-backup").await;
    let service = SqliteBackupService::new(artifacts, &database);
    let backup = directory.path().join("backup");
    let key = ArtifactIdempotencyKey::new("backup:sqlite-online").unwrap();

    let snapshot = service.prepare(&backup, key.clone()).await.unwrap();
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].artifact, artifact);
    let manifest = SqliteBackupService::inspect(&backup).unwrap();
    assert_eq!(manifest.protection_key, key);
    assert_eq!(manifest.artifact_records, snapshot.records);

    connection
        .execute(
            "INSERT INTO journal(position, payload) VALUES (2, 'after-snapshot')",
            [],
        )
        .unwrap();
    assert_eq!(service.prepare(&backup, key).await.unwrap(), snapshot);

    let restored = directory.path().join("restored.sqlite3");
    SqliteBackupService::restore_to(&backup, &restored).unwrap();
    let restored = rusqlite::Connection::open(restored).unwrap();
    let rows: i64 = restored
        .query_row("SELECT COUNT(*) FROM journal", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    let payload: String = restored
        .query_row(
            "SELECT payload FROM journal WHERE position = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(payload, "before-snapshot");
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_backup_fails_closed_when_prior_capture_has_no_database() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("made.sqlite3");
    rusqlite::Connection::open(&database).unwrap();
    let artifacts = LocalArtifactStore::open(directory.path().join("artifacts")).unwrap();
    let artifact_identity = artifacts.store_identity().await.unwrap();
    let service = SqliteBackupService::new(artifacts, &database);
    let backup = directory.path().join("backup");
    fs::create_dir_all(&backup).unwrap();
    let key = ArtifactIdempotencyKey::new("backup:interrupted-sqlite").unwrap();
    let identity = digest(
        fs::canonicalize(&database)
            .unwrap()
            .to_string_lossy()
            .as_bytes(),
    );
    fs::write(
        backup.join("sqlite-owner.json"),
        serde_json::to_vec(&(key.clone(), identity, artifact_identity)).unwrap(),
    )
    .unwrap();

    assert_eq!(
        service.prepare(&backup, key).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_retry_cannot_mix_captured_database_with_another_artifact_store() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("made.sqlite3");
    rusqlite::Connection::open(&database).unwrap();
    let first_store = LocalArtifactStore::open(directory.path().join("artifacts-a")).unwrap();
    upload(&first_store, b"store-a", "store-a").await;
    let backup = directory.path().join("backup");
    let key = ArtifactIdempotencyKey::new("backup:captured-db-store-a").unwrap();
    SqliteBackupService::new(first_store, &database)
        .prepare(&backup, key.clone())
        .await
        .unwrap();
    fs::remove_file(backup.join("sqlite-manifest.json")).unwrap();

    let second_store = LocalArtifactStore::open(directory.path().join("artifacts-b")).unwrap();
    upload(&second_store, b"store-b", "store-b").await;
    assert_eq!(
        SqliteBackupService::new(second_store, &database)
            .prepare(&backup, key)
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
}

#[cfg(feature = "sqlite")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)] // One acceptance flow keeps snapshot, pins, GC and restore race correlated.
async fn sqlite_set_keeps_database_and_exact_blobs_together_under_writes_and_restores_atomically() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("made.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE journal(position INTEGER PRIMARY KEY, payload TEXT NOT NULL);
             INSERT INTO journal VALUES (1, 'boundary');
             CREATE TABLE snapshot_padding(bytes BLOB NOT NULL);
             INSERT INTO snapshot_padding VALUES (zeroblob(8388608));",
        )
        .unwrap();
    drop(connection);
    let store_root = directory.path().join("artifact-store");
    let store = LocalArtifactStore::open(&store_root).unwrap();
    let artifact = upload(&store, b"composed snapshot blob", "composed-snapshot").await;
    let retired_first = upload(&store, b"retired snapshot blob one", "retired-one").await;
    let retired_second = upload(&store, b"retired snapshot blob two", "retired-two").await;
    let receipt_key = ArtifactIdempotencyKey::new("receipt:composed-snapshot").unwrap();
    store
        .protect_references(receipt_key.clone(), vec![artifact.artifact_id().clone()])
        .await
        .unwrap();
    for retired in [&artifact, &retired_first, &retired_second] {
        store
            .tombstone(TombstoneArtifact {
                artifact_id: retired.artifact_id().clone(),
                actor: ArtifactRetentionActor::new("acceptance").unwrap(),
                policy: ArtifactRetentionPolicy::new("restore-set").unwrap(),
                retired_at: OffsetDateTime::UNIX_EPOCH,
            })
            .await
            .unwrap();
    }
    let service = SqliteArtifactBackupService::new(store.clone(), &database);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (snapshot_barrier, writer_barrier) = std::sync::mpsc::sync_channel(0);
    let committed_writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer = spawn_sqlite_backup_writer(
        database.clone(),
        stop.clone(),
        writer_barrier,
        committed_writes.clone(),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while committed_writes.load(std::sync::atomic::Ordering::Acquire) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("writer must commit at least once before the snapshot starts");
    let writes_before_snapshot = committed_writes.load(std::sync::atomic::Ordering::Acquire);

    let backup = directory.path().join("backup-set");
    let backup_started = Instant::now();
    let backup_task = tokio::spawn({
        let service = service.clone();
        let backup = backup.clone();
        async move {
            service
                .backup_to(
                    backup,
                    ArtifactIdempotencyKey::new("backup:sqlite-set-under-write").unwrap(),
                )
                .await
        }
    });
    let snapshot_directory = backup.join("database");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let copy_started = fs::read_dir(&snapshot_directory).is_ok_and(|entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("database.tmp-")
                })
            });
            if copy_started {
                break;
            }
            assert!(
                !backup_task.is_finished(),
                "SQLite snapshot completed before the concurrent-write barrier"
            );
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("SQLite online copy must expose its in-progress snapshot");
    snapshot_barrier
        .send(())
        .expect("writer must still be waiting at the snapshot barrier");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while committed_writes.load(std::sync::atomic::Ordering::Acquire) <= writes_before_snapshot
        {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("writer must commit while the SQLite snapshot is in progress");
    assert!(
        !backup_task.is_finished(),
        "the observed commit must precede snapshot completion"
    );
    let writes_during_snapshot =
        committed_writes.load(std::sync::atomic::Ordering::Acquire) - writes_before_snapshot;
    stop.store(true, std::sync::atomic::Ordering::Release);
    writer.await.unwrap();
    let manifest = backup_task.await.unwrap().unwrap();
    let backup_elapsed = backup_started.elapsed();
    let source_frontier: i64 = rusqlite::Connection::open(&database)
        .unwrap()
        .query_row("SELECT MAX(position) FROM journal", [], |row| row.get(0))
        .unwrap();
    service.verify(&backup, &manifest).unwrap();
    let retried = service
        .backup_to(
            &backup,
            ArtifactIdempotencyKey::new("backup:sqlite-set-under-write").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retried, manifest);
    assert_eq!(
        service
            .backup_to(
                &backup,
                ArtifactIdempotencyKey::new("backup:different-key").unwrap()
            )
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );
    let other_database = directory.path().join("other.sqlite3");
    rusqlite::Connection::open(&other_database).unwrap();
    let other_source = SqliteArtifactBackupService::new(store.clone(), other_database);
    assert_eq!(
        other_source
            .backup_to(
                &backup,
                ArtifactIdempotencyKey::new("backup:sqlite-set-under-write").unwrap()
            )
            .await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );

    let restored = directory.path().join("restored-set");
    let restore_started = Instant::now();
    service.restore_set_to(&backup, &restored).await.unwrap();
    let restore_elapsed = restore_started.elapsed();
    assert!(restored.join("restore-complete.json").exists());
    let restored_database = rusqlite::Connection::open(restored.join("database.sqlite3")).unwrap();
    let rows: i64 = restored_database
        .query_row("SELECT MAX(position) FROM journal", [], |row| row.get(0))
        .unwrap();
    assert!(
        rows > i64::try_from(writes_before_snapshot).unwrap(),
        "snapshot omitted a journal commit completed before capture"
    );
    assert!((1..=source_frontier).contains(&rows));
    let restored_store = LocalArtifactStore::open(restored.join("artifacts")).unwrap();
    for expected in [&artifact, &retired_first, &retired_second] {
        assert_eq!(
            restored_store
                .get(expected.artifact_id())
                .await
                .unwrap()
                .artifact,
            *expected
        );
        assert!(restored_store
            .backup_content_available(expected.artifact_id())
            .await
            .unwrap());
    }
    let restored_protections = restored_store.active_protections().await.unwrap();
    assert!(restored_protections
        .iter()
        .any(|protection| protection.key == receipt_key));
    assert!(restored_protections
        .iter()
        .all(|protection| !protection.key.as_str().starts_with("restore:")));
    let gc_preview = restored_store
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("restored-set-gc", 259_200_000),
        )
        .await
        .unwrap();
    assert_eq!(
        gc_preview.candidates.len(),
        2,
        "two retired blobs remain collectable"
    );
    assert!(gc_preview.exclusions.iter().any(|exclusion| {
        exclusion.digest == *artifact.digest()
            && exclusion
                .reasons
                .contains(&ArtifactGcExclusionReason::ProtectedReference)
    }));
    assert_eq!(
        service.restore_set_to(&backup, &restored).await,
        Err(ArtifactStoreError::IdempotencyConflict)
    );

    let raced = directory.path().join("raced-restore-set");
    let (left, right) = tokio::join!(
        service.restore_set_to(&backup, &raced),
        service.restore_set_to(&backup, &raced),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(
        (left, right),
        (Ok(()), Err(ArtifactStoreError::IdempotencyConflict))
            | (Err(ArtifactStoreError::IdempotencyConflict), Ok(()))
    ));
    assert!(raced.join("restore-complete.json").exists());
    println!(
        "{}",
        serde_json::json!({
            "backend":"sqlite",
            "sample":"backup_set_under_writer_and_restore_race",
            "backup_ms":backup_elapsed.as_secs_f64()*1000.0,
            "restore_ms":restore_elapsed.as_secs_f64()*1000.0,
            "rto_ms":restore_elapsed.as_secs_f64()*1000.0,
            "source_frontier":source_frontier,
            "restored_frontier":rows,
            "writes_before_snapshot":writes_before_snapshot,
            "writes_during_snapshot":writes_during_snapshot,
            "lost_commits":source_frontier-rows,
            "rpo_commits":source_frontier-rows,
            "receipt_protection":receipt_key.as_str(),
            "retired_digests":[retired_first.digest(), retired_second.digest()],
            "gc_protected_reference":artifact.digest(),
            "restore_race":{"successes":1,"conflicts":1}
        })
    );
}

#[cfg(feature = "sqlite")]
fn spawn_sqlite_backup_writer(
    database: std::path::PathBuf,
    stop: Arc<std::sync::atomic::AtomicBool>,
    snapshot_barrier: std::sync::mpsc::Receiver<()>,
    committed_writes: Arc<std::sync::atomic::AtomicUsize>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let connection = rusqlite::Connection::open(database).unwrap();
        let mut position = 2_i64;
        connection
            .execute(
                "INSERT INTO journal(position, payload) VALUES (?1, 'before-backup')",
                [position],
            )
            .unwrap();
        committed_writes.fetch_add(1, std::sync::atomic::Ordering::Release);
        position += 1;
        if snapshot_barrier.recv().is_err() {
            return;
        }
        while !stop.load(std::sync::atomic::Ordering::Acquire) {
            connection
                .execute(
                    "INSERT INTO journal(position, payload) VALUES (?1, 'during-backup')",
                    [position],
                )
                .unwrap();
            committed_writes.fetch_add(1, std::sync::atomic::Ordering::Release);
            position += 1;
        }
    })
}

#[tokio::test]
async fn corrupt_protection_fails_gc_closed() {
    let directory = TempDir::new().unwrap();
    let store = LocalArtifactStore::open(directory.path()).unwrap();
    let artifact = upload(&store, b"corrupt protection", "corrupt-protection").await;
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:retention").unwrap(),
            policy: ArtifactRetentionPolicy::new("corrupt-protection").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    store
        .protect_references(
            ArtifactIdempotencyKey::new("backup:corrupt-protection").unwrap(),
            vec![artifact.artifact_id().clone()],
        )
        .await
        .unwrap();
    let protection = fs::read_dir(directory.path().join("protections"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(protection, b"not json").unwrap();

    assert_eq!(
        store
            .plan_gc(
                OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
                lease("corrupt-protection-gc", 259_200_000),
            )
            .await
            .unwrap_err()
            .to_string(),
        "artifact storage is unavailable: serialize or decode artifact metadata: expected ident at line 1 column 2"
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn retention_is_dry_run_first_and_gc_requires_an_unexpired_lease() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let old = upload_at(
        &store,
        b"old retention body",
        "retention-old",
        OffsetDateTime::UNIX_EPOCH,
    )
    .await;
    let fresh = upload_at(
        &store,
        b"fresh retention body",
        "retention-fresh",
        OffsetDateTime::UNIX_EPOCH + time::Duration::days(10),
    )
    .await;
    let retention = ArtifactRetentionService::new(store.clone());
    let plan = retention
        .plan_retention(
            ArtifactRetentionActor::new("host:retention").unwrap(),
            ArtifactRetentionPolicy::new("expired-artifacts").unwrap(),
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            OffsetDateTime::UNIX_EPOCH,
            lease("retention-plan", 259_200_000),
        )
        .await
        .unwrap();
    let dry = ArtifactRetentionService::<LocalArtifactStore>::dry_run(&plan);
    assert!(dry.dry_run);
    assert_eq!(dry.planned, vec![old.artifact_id().clone()]);
    assert!(store
        .get(old.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_none());

    let applied = retention
        .apply(
            &plan,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            None,
        )
        .await
        .unwrap();
    assert_eq!(applied.applied.len(), 1);
    assert!(store
        .get(fresh.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_none());
    let retried = retention
        .apply(
            &plan,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            None,
        )
        .await
        .unwrap();
    assert!(retried.applied.is_empty());
    assert_eq!(retried.already_retired, vec![old.artifact_id().clone()]);

    let blob = directory
        .path()
        .join("blobs")
        .join(old.digest().as_str().trim_start_matches("sha256:"));
    assert!(blob.exists());
    let gc_plan = store
        .plan_gc(
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            lease("gc-plan", 259_200_000),
        )
        .await
        .unwrap();
    let gc_dry = LocalArtifactStore::dry_run_gc(&gc_plan);
    assert!(gc_dry.dry_run);
    assert_eq!(gc_dry.planned, vec![old.digest().clone()]);
    assert!(blob.exists());
    let gc = store
        .apply_gc(
            &gc_plan,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(2),
        )
        .await
        .unwrap();
    assert_eq!(gc.deleted, vec![old.digest().clone()]);
    assert!(!blob.exists());
    assert!(store
        .get(old.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_some());

    let mut expired = gc_plan.clone();
    expired.lease = lease("expired-gc", 1);
    assert_eq!(
        store
            .apply_gc(
                &expired,
                OffsetDateTime::UNIX_EPOCH + time::Duration::days(2)
            )
            .await,
        Err(ArtifactStoreError::AccessDenied)
    );
}
