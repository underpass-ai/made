use made_app::artifacts::ArtifactCursor;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
};
use made_embedded::EmbeddedMade;
use time::OffsetDateTime;

const BYTES: &[u8] = b"fixture";
const DIGEST: &str = "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d";

#[tokio::test]
async fn artifact_upload_and_cursor_survive_reopening_through_the_public_facade() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("made.sqlite3");
    let made = EmbeddedMade::open(&path).unwrap();
    let status = made
        .begin_artifact_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: ArtifactDigest::new(DIGEST).unwrap(),
            size_bytes: ArtifactSizeBytes::new(BYTES.len() as u64),
            media_type: ArtifactMediaType::new("text/plain").unwrap(),
            provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
            idempotency_key: ArtifactIdempotencyKey::new("embedded-public-restart").unwrap(),
        })
        .await
        .unwrap();
    made.put_artifact_chunk(PutArtifactChunk {
        upload_id: status.upload_id.clone(),
        offset: ArtifactByteOffset::ZERO,
        bytes: BYTES.to_vec(),
        chunk_digest: ArtifactDigest::new(DIGEST).unwrap(),
    })
    .await
    .unwrap();
    let artifact = made
        .commit_artifact_upload(&status.upload_id)
        .await
        .unwrap();
    let artifact_id = artifact.artifact_id().clone();
    drop(made);

    let reopened = EmbeddedMade::open(&path).unwrap();
    assert_eq!(
        reopened.get_artifact(&artifact_id).await.unwrap().artifact,
        artifact
    );
    let chunk = reopened
        .read_artifact_chunk(ReadArtifactChunk {
            artifact_id: artifact_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            max_bytes: ArtifactChunkLimit::DEFAULT,
        })
        .await
        .unwrap();
    assert_eq!(chunk.bytes, BYTES);
    assert!(chunk.is_complete());

    let page = reopened
        .list_artifacts(None, ArtifactPageLimit::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let cursor = ArtifactCursor::after(&artifact_id);
    assert!(reopened
        .list_artifacts(Some(&cursor), ArtifactPageLimit::new(1).unwrap())
        .await
        .unwrap()
        .items
        .is_empty());
}
