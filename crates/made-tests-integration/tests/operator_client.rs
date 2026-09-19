use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_client::{MadeClient, ProgressCheckpoint};
use made_proto::v1 as pb;
use pb::made_service_client::MadeServiceClient;
use prost_types::Timestamp;

use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};

const CEREMONY: &str = r#"
version: "1.0"
name: operator_client
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [work_done]
steps:
  - {id: work, state: OPEN, handler: host_callback}
guards:
  work_done: {type: automated, check: "step_status:work:COMPLETED"}
roles:
  - {id: WORKER, allowed_actions: [work, finish]}
"#;

const BYTES: &[u8] = b"fixture";
const DIGEST: &str = "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d";

#[tokio::test]
async fn client_reconnects_from_checkpoint_and_preserves_late_terminal_history() {
    let fixture = GrpcFixture::start().await;
    let endpoint = format!("http://{}", fixture.addr);
    let mut rpc = MadeServiceClient::new(fixture.channel.clone());
    rpc.start_ceremony(pb::StartCeremonyRequest {
        ceremony_id: "operator-reconnect".to_owned(),
        definition_yaml: CEREMONY.to_owned(),
        context: None,
        actor_id: "operator-test".to_owned(),
        actor_kind: "service".to_owned(),
    })
    .await
    .unwrap();

    let client = MadeClient::connect(&endpoint).await.unwrap();
    let first = client
        .watch_once(
            &ProgressCheckpoint::new("operator-reconnect", 0),
            1,
            Some(0),
        )
        .await
        .unwrap();
    assert_eq!(first.records().len(), 1);
    assert_eq!(first.checkpoint().after_sequence(), 1);
    drop(client);

    let claim = rpc
        .claim_ceremony_step(pb::ClaimCeremonyStepRequest {
            ceremony_id: "operator-reconnect".to_owned(),
            step_id: "work".to_owned(),
            actor_kind: "agent".to_owned(),
            lease_owner_id: "operator-worker".to_owned(),
            idempotency_key: "operator-claim".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .unwrap()
        .into_inner();
    let client = MadeClient::connect(&endpoint).await.unwrap();
    let cancelled = client
        .cancel(pb::CancelCeremonyRequest {
            ceremony_id: "operator-reconnect".to_owned(),
            actor_id: "operator-test".to_owned(),
            actor_kind: "service".to_owned(),
            reason: "stop requested".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(cancelled.lifecycle, "ended");
    assert_eq!(cancelled.end_reason, "cancelled");
    rpc.complete_ceremony_step(pb::CompleteCeremonyStepRequest {
        ceremony_id: "operator-reconnect".to_owned(),
        step_id: "work".to_owned(),
        actor_kind: "agent".to_owned(),
        status: "completed".to_owned(),
        output: None,
        error: String::new(),
        claim_fence: claim.claim_fence,
    })
    .await
    .unwrap();

    let terminal = client
        .watch_once(first.checkpoint(), 100, Some(0))
        .await
        .unwrap();
    let terminal_event_types: Vec<_> = terminal
        .records()
        .iter()
        .map(|record| record.event_type.as_str())
        .collect();
    assert!(terminal_event_types.contains(&"ceremony_cancelled"));
    assert!(!terminal_event_types.contains(&"late_step_result_observed"));
    assert_eq!(
        terminal.end_reason(),
        pb::StreamCeremonyEndReason::Terminal,
        "events: {terminal_event_types:?}"
    );
    assert!(terminal.checkpoint().after_sequence() < terminal.head_sequence());

    let late = client
        .watch_once(terminal.checkpoint(), 100, Some(0))
        .await
        .unwrap();
    let late_event_types: Vec<_> = late
        .records()
        .iter()
        .map(|record| record.event_type.as_str())
        .collect();
    assert_eq!(late_event_types, vec!["late_step_result_observed"]);
    assert_eq!(late.end_reason(), pb::StreamCeremonyEndReason::Terminal);
    assert_eq!(late.checkpoint().after_sequence(), late.head_sequence());
    let state = client.get_ceremony("operator-reconnect").await.unwrap();
    assert_eq!(state.lifecycle, "ended");
    assert_eq!(state.end_reason, "cancelled");
}

#[tokio::test]
async fn client_pages_metadata_and_exports_verified_artifact_chunks() {
    let scratch = scratch();
    let store = Arc::new(LocalArtifactStore::open(scratch.path().join("store")).unwrap());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_artifact_store(store)).await;
    let mut rpc = MadeServiceClient::new(fixture.channel);
    upload(&mut rpc, "artifact-operator-a", "operator-upload-a").await;
    upload(&mut rpc, "artifact-operator-b", "operator-upload-b").await;

    let client = MadeClient::connect(format!("http://{}", fixture.addr))
        .await
        .unwrap();
    let first = client.list_artifacts(None, 1).await.unwrap();
    assert_eq!(first.artifacts.len(), 1);
    let second = client.list_artifacts(first.next_cursor, 1).await.unwrap();
    assert_eq!(second.artifacts.len(), 1);
    assert!(second.next_cursor.is_none());

    let destination = scratch.path().join("export/fixture.txt");
    client
        .export_artifact("artifact-operator-a", &destination, false)
        .await
        .unwrap();
    assert_eq!(tokio::fs::read(destination).await.unwrap(), BYTES);
}

async fn upload(client: &mut MadeServiceClient<tonic::transport::Channel>, id: &str, key: &str) {
    let upload = client
        .begin_artifact_upload(pb::BeginArtifactUploadRequest {
            requested_artifact_id: Some(id.to_owned()),
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
            idempotency_key: key.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .upload
        .unwrap();
    client
        .put_artifact_chunk(pb::PutArtifactChunkRequest {
            upload_id: upload.upload_id.clone(),
            offset: 0,
            bytes: BYTES.to_vec(),
            chunk_digest: DIGEST.to_owned(),
        })
        .await
        .unwrap();
    client
        .commit_artifact_upload(pb::CommitArtifactUploadRequest {
            upload_id: upload.upload_id,
        })
        .await
        .unwrap();
}

fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
