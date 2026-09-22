use std::fs;

use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactRetentionActor, ArtifactRetentionPolicy,
    BeginArtifactUpload, PutArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::OffsetDateTime;

use super::local_artifact_io::read_json;
use super::local_artifact_repository::LocalArtifactRepository;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

fn request(bytes: &[u8], key: &str, requested: Option<ArtifactId>) -> BeginArtifactUpload {
    BeginArtifactUpload {
        requested_artifact_id: requested,
        expected_digest: digest(bytes),
        size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
        media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
        provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
        idempotency_key: ArtifactIdempotencyKey::new(key).unwrap(),
    }
}

fn staged(
    repository: &LocalArtifactRepository,
    bytes: &[u8],
    key: &str,
    requested: Option<ArtifactId>,
) -> made_core::ports::ArtifactUploadId {
    let status = repository.begin(request(bytes, key, requested)).unwrap();
    repository
        .put(&PutArtifactChunk {
            upload_id: status.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            bytes: bytes.to_vec(),
            chunk_digest: digest(bytes),
        })
        .unwrap();
    status.upload_id
}

#[test]
fn upload_manifest_resolves_its_authoritative_artifact_identity() {
    let directory = TempDir::new().unwrap();
    let repository = LocalArtifactRepository::open(directory.path()).unwrap();
    let requested = ArtifactId::new("artifact-requested").unwrap();
    let requested_upload = repository
        .begin(request(
            b"requested",
            "requested-id",
            Some(requested.clone()),
        ))
        .unwrap();
    assert_eq!(
        repository
            .artifact_id_for_upload(&requested_upload.upload_id)
            .unwrap(),
        requested
    );

    let generated_upload = repository
        .begin(request(b"generated", "generated-id", None))
        .unwrap();
    assert_eq!(
        repository
            .artifact_id_for_upload(&generated_upload.upload_id)
            .unwrap()
            .as_str(),
        format!(
            "artifact-{}",
            generated_upload
                .upload_id
                .as_str()
                .trim_start_matches("upload-")
        )
    );
}

#[test]
fn every_commit_phase_is_recoverable_after_an_injected_failure() {
    for phase in ["blob_published", "record_published", "manifest_published"] {
        let directory = TempDir::new().unwrap();
        let repository = LocalArtifactRepository::open(directory.path()).unwrap();
        let upload = staged(&repository, b"recover every phase", phase, None);
        assert_eq!(
            repository.commit_observing(&upload, |observed| {
                if observed == phase {
                    Err(made_core::ports::ArtifactStoreError::StorageUnavailable {
                        detail: String::new(),
                    })
                } else {
                    Ok(())
                }
            }),
            Err(made_core::ports::ArtifactStoreError::StorageUnavailable {
                detail: String::new()
            }),
            "phase {phase} did not inject"
        );
        if phase == "record_published" {
            assert_eq!(
                repository.abort(&upload),
                Err(made_core::ports::ArtifactStoreError::UploadCommitted)
            );
        }
        drop(repository);

        let reopened = LocalArtifactRepository::open(directory.path()).unwrap();
        let artifact = reopened.commit(&upload).unwrap();
        assert_eq!(
            reopened.get(artifact.artifact_id()).unwrap().artifact,
            artifact
        );
    }
}

#[test]
fn recommit_of_the_same_requested_id_preserves_its_tombstone() {
    let directory = TempDir::new().unwrap();
    let repository = LocalArtifactRepository::open(directory.path()).unwrap();
    let artifact_id = ArtifactId::new("artifact-stable").unwrap();
    let first = staged(
        &repository,
        b"retained bytes",
        "tombstone-first",
        Some(artifact_id.clone()),
    );
    repository.commit(&first).unwrap();
    let tombstone = repository
        .tombstone(TombstoneArtifact {
            artifact_id: artifact_id.clone(),
            actor: ArtifactRetentionActor::new("host:test").unwrap(),
            policy: ArtifactRetentionPolicy::new("expired").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .unwrap();
    let second = staged(
        &repository,
        b"retained bytes",
        "tombstone-second",
        Some(artifact_id.clone()),
    );
    repository.commit(&second).unwrap();
    assert_eq!(
        repository.get(&artifact_id).unwrap().tombstone,
        Some(tombstone)
    );
}

#[test]
fn corrupt_existing_record_is_never_treated_as_absent() {
    let directory = TempDir::new().unwrap();
    let repository = LocalArtifactRepository::open(directory.path()).unwrap();
    let artifact_id = ArtifactId::new("artifact-corrupt-record").unwrap();
    let first = staged(
        &repository,
        b"record body",
        "record-first",
        Some(artifact_id.clone()),
    );
    repository.commit(&first).unwrap();
    let record_path = repository.layout.artifact(&artifact_id);
    fs::write(&record_path, b"not-json").unwrap();

    let second = staged(
        &repository,
        b"record body",
        "record-second",
        Some(artifact_id),
    );
    assert!(matches!(
        repository.commit(&second),
        Err(made_core::ports::ArtifactStoreError::StorageUnavailable { .. })
    ));
    assert!(read_json::<made_core::ports::ArtifactRecord>(&record_path).is_err());
}
