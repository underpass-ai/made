#![cfg(feature = "container-tests")]

use std::sync::Arc;
use std::time::Duration;

use made_adapters::artifacts::{ArtifactBackupService, LocalArtifactStore};
use made_adapters::postgres::{PostgresArtifactStore, PostgresConfig, PostgresPool};
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactRetentionActor,
    ArtifactRetentionPolicy, ArtifactStoreError, ArtifactStorePort, BeginArtifactUpload,
    PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact, ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactRef, ArtifactSizeBytes,
};
use sha2::{Digest, Sha256};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
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

async fn postgres() -> (
    PostgresPool,
    String,
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
    let mut config = PostgresConfig::from_url(url.clone());
    config.acquire_timeout = Duration::from_secs(10);
    let mut last = None;
    for _ in 0..20 {
        match PostgresPool::connect(&config).await {
            Ok(pool) => {
                pool.run_migrations().await.unwrap();
                return (pool, url, container);
            }
            Err(error) => {
                last = Some(error);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    panic!("postgres did not start: {last:?}");
}

#[tokio::test]
async fn postgres_chunks_resume_deduplicate_verify_and_enforce_limits() {
    let (pool, url, _container) = postgres().await;
    let left = vec![11_u8; 80_000];
    let right = vec![22_u8; 90_000];
    let bytes = [left.as_slice(), right.as_slice()].concat();
    let (store, artifact) = resume_and_commit(pool, &url, &bytes, left, right).await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    assert_deduplicated(&store, &raw, &bytes, &artifact).await;
    assert_incomplete_and_limits(&store, &artifact).await;
    assert_tombstone_survives_recommit(&store, &bytes, &artifact).await;
    assert_backup_restore_and_tamper(store, &raw, &artifact).await;
}

async fn resume_and_commit(
    pool: PostgresPool,
    url: &str,
    bytes: &[u8],
    left: Vec<u8>,
    right: Vec<u8>,
) -> (Arc<PostgresArtifactStore>, ArtifactRef) {
    let first_store = PostgresArtifactStore::new(pool);
    let status = first_store
        .begin_upload(begin(bytes, "postgres-restart"))
        .await
        .unwrap();
    first_store
        .put_chunk(PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest(&left),
            bytes: left.clone(),
        })
        .await
        .unwrap();
    drop(first_store);

    let second_pool = PostgresPool::connect(&PostgresConfig::from_url(url))
        .await
        .unwrap();
    let store = Arc::new(PostgresArtifactStore::new(second_pool));
    let resumed = store
        .begin_upload(begin(bytes, "postgres-restart"))
        .await
        .unwrap();
    assert_eq!(resumed.next_offset.get(), left.len() as u64);
    store
        .put_chunk(PutArtifactChunk {
            upload_id: resumed.upload_id.clone(),
            offset: ArtifactByteOffset::new(left.len() as u64),
            chunk_digest: digest(&right),
            bytes: right,
        })
        .await
        .unwrap();
    let artifact = store.commit_upload(&resumed.upload_id).await.unwrap();
    let crossing = store
        .read_chunk(ReadArtifactChunk {
            artifact_id: artifact.artifact_id().clone(),
            offset: ArtifactByteOffset::new(79_990),
            max_bytes: ArtifactChunkLimit::new(40).unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(crossing.bytes, bytes[79_990..80_030]);
    (store, artifact)
}

async fn assert_deduplicated(
    store: &PostgresArtifactStore,
    raw: &sqlx::PgPool,
    bytes: &[u8],
    artifact: &ArtifactRef,
) {
    let duplicate = store
        .begin_upload(begin(bytes, "postgres-dedup"))
        .await
        .unwrap();
    for (offset, chunk) in [(0_u64, &bytes[..80_000]), (80_000_u64, &bytes[80_000..])] {
        store
            .put_chunk(PutArtifactChunk {
                upload_id: duplicate.upload_id.clone(),
                offset: ArtifactByteOffset::new(offset),
                chunk_digest: digest(chunk),
                bytes: chunk.to_vec(),
            })
            .await
            .unwrap();
    }
    let duplicate_ref = store.commit_upload(&duplicate.upload_id).await.unwrap();
    assert_eq!(duplicate_ref.digest(), artifact.digest());
    assert_ne!(duplicate_ref.artifact_id(), artifact.artifact_id());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifact_blobs WHERE digest = $1")
        .bind(artifact.digest().as_str())
        .fetch_one(raw)
        .await
        .unwrap();
    assert_eq!(
        count, 3,
        "same content must reuse the three canonical stored chunks"
    );
}

async fn assert_incomplete_and_limits(store: &PostgresArtifactStore, artifact: &ArtifactRef) {
    let incomplete = store
        .begin_upload(begin(b"four", "postgres-incomplete"))
        .await
        .unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: incomplete.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest(b"fo"),
            bytes: b"fo".to_vec(),
        })
        .await
        .unwrap();
    assert!(matches!(
        store.commit_upload(&incomplete.upload_id).await,
        Err(ArtifactStoreError::Incomplete { .. })
    ));
    let _ = artifact;
    assert!(ArtifactChunkLimit::new(ARTIFACT_MAX_CHUNK_BYTES + 1).is_err());
}

async fn assert_tombstone_survives_recommit(
    store: &PostgresArtifactStore,
    bytes: &[u8],
    artifact: &ArtifactRef,
) {
    let tombstone = store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("host:postgres-test").unwrap(),
            policy: ArtifactRetentionPolicy::new("expired").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
    let mut same_id = begin(bytes, "postgres-same-id");
    same_id.requested_artifact_id = Some(artifact.artifact_id().clone());
    let upload = store.begin_upload(same_id).await.unwrap();
    store
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest(bytes),
            bytes: bytes.to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(
        store.commit_upload(&upload.upload_id).await.unwrap(),
        *artifact
    );
    assert_eq!(
        store.get(artifact.artifact_id()).await.unwrap().tombstone,
        Some(tombstone)
    );
}

async fn assert_backup_restore_and_tamper(
    store: Arc<PostgresArtifactStore>,
    raw: &sqlx::PgPool,
    artifact: &ArtifactRef,
) {
    let backup = tempfile::tempdir().unwrap().path().join("unused");
    let stable_backup_root = tempfile::tempdir().unwrap();
    let stable_backup = stable_backup_root.path().join("backup");
    ArtifactBackupService::new(store.clone())
        .backup_to(&stable_backup)
        .await
        .unwrap();
    let restored_root = tempfile::tempdir().unwrap();
    let local = Arc::new(LocalArtifactStore::open(restored_root.path()).unwrap());
    ArtifactBackupService::new(local.clone())
        .restore_from(&stable_backup)
        .await
        .unwrap();
    assert_eq!(
        local.get(artifact.artifact_id()).await.unwrap().artifact,
        artifact.clone()
    );

    let tampered = vec![99_u8; 64 * 1024];
    sqlx::query("UPDATE artifact_blobs SET bytes = $1 WHERE digest = $2 AND chunk_offset = 0")
        .bind(tampered)
        .bind(artifact.digest().as_str())
        .execute(raw)
        .await
        .unwrap();
    assert_eq!(
        ArtifactBackupService::new(store).backup_to(backup).await,
        Err(ArtifactStoreError::InvalidBackup)
    );
}
