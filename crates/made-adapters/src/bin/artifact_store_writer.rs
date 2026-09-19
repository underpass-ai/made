//! Helper process proving independent hosts serialize one local artifact store.

use made_adapters::artifacts::LocalArtifactStore;
use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, PutArtifactChunk,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = std::env::args().nth(1).expect("artifact store path");
    let bytes = b"two process artifact";
    let store = LocalArtifactStore::open(root).expect("store opens");
    let status = store
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: digest(bytes),
            size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
            media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
            provenance: ArtifactProvenance::generated_report(OffsetDateTime::UNIX_EPOCH),
            idempotency_key: ArtifactIdempotencyKey::new("two-process-key").unwrap(),
        })
        .await
        .expect("begin succeeds");
    if status.next_offset == ArtifactByteOffset::ZERO {
        let result = store
            .put_chunk(PutArtifactChunk {
                upload_id: status.upload_id.clone(),
                offset: ArtifactByteOffset::ZERO,
                bytes: bytes.to_vec(),
                chunk_digest: digest(bytes),
            })
            .await;
        assert!(
            result.is_ok() || result == Err(ArtifactStoreError::UploadCommitted),
            "idempotent put failed: {result:?}"
        );
    }
    let artifact = store
        .commit_upload(&status.upload_id)
        .await
        .expect("commit succeeds");
    println!("{}", artifact.artifact_id());
}
