use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_proto::v1 as pb;
use pb::made_service_client::MadeServiceClient;
use prost_types::Timestamp;

use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};

const BYTES: &[u8] = b"fixture";
const DIGEST: &str = "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d";

async fn commit_fixture(
    client: &mut MadeServiceClient<tonic::transport::Channel>,
) -> pb::ArtifactReference {
    let upload = client
        .begin_artifact_upload(pb::BeginArtifactUploadRequest {
            requested_artifact_id: None,
            expected_digest: DIGEST.to_owned(),
            size_bytes: BYTES.len() as u64,
            media_type: "text/plain".to_owned(),
            provenance: Some(pb::ArtifactProvenance {
                source_kind: pb::ArtifactSourceKind::GeneratedReport as i32,
                execution_receipt_id: None,
                operation_id: None,
                accepted_claim_fence: None,
                observed_at: Some(Timestamp {
                    seconds: 0,
                    nanos: 0,
                }),
                import_ref: None,
            }),
            idempotency_key: "grpc-artifact-test".to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .upload
        .unwrap();
    let progress = client
        .put_artifact_chunk(pb::PutArtifactChunkRequest {
            upload_id: upload.upload_id.clone(),
            offset: 0,
            bytes: BYTES.to_vec(),
            chunk_digest: DIGEST.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .upload
        .unwrap();
    assert_eq!(progress.next_offset, BYTES.len() as u64);

    let artifact = client
        .commit_artifact_upload(pb::CommitArtifactUploadRequest {
            upload_id: upload.upload_id,
        })
        .await
        .unwrap()
        .into_inner()
        .artifact
        .unwrap();
    assert_eq!(artifact.digest, DIGEST);
    artifact
}

#[tokio::test]
async fn bounded_artifact_lifecycle_crosses_the_public_grpc_surface() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_artifact_store(store)).await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    let artifact = commit_fixture(&mut client).await;

    let listed = client
        .list_artifacts(pb::ListArtifactsRequest {
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(listed.artifacts.len(), 1);
    assert_eq!(
        listed.artifacts[0]
            .authorization
            .as_ref()
            .unwrap()
            .principal_id,
        "grpc-fixture-host"
    );

    let chunk = client
        .read_artifact_chunk(pb::ReadArtifactChunkRequest {
            artifact_id: artifact.artifact_id.clone(),
            offset: 0,
            max_bytes: 4,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(chunk.bytes, b"fixt");
    assert_eq!(chunk.next_offset, 4);
    assert!(!chunk.eof);

    let tombstone = client
        .tombstone_artifact(pb::TombstoneArtifactRequest {
            artifact_id: artifact.artifact_id.clone(),
            actor: "grpc-test-host".to_owned(),
            policy: "test-retention".to_owned(),
            retired_at: Some(Timestamp {
                seconds: 1,
                nanos: 0,
            }),
        })
        .await
        .unwrap()
        .into_inner()
        .tombstone
        .unwrap();
    assert_eq!(tombstone.digest, DIGEST);
    assert_eq!(
        tombstone.authorization.unwrap().principal_id,
        "grpc-fixture-host"
    );
    assert!(client
        .read_artifact_chunk(pb::ReadArtifactChunkRequest {
            artifact_id: artifact.artifact_id,
            offset: 0,
            max_bytes: 4,
        })
        .await
        .is_err());
}
