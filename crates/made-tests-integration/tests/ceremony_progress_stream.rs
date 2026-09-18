//! Progress is a resumable stream across the direct RPC and durable facade.

use futures::StreamExt;
use made_app::usecases::StreamCeremonyInput;
use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, CeremonyProgressWait, StreamVersion,
};
use made_embedded::EmbeddedMade;
use made_mcp::{EmbeddedMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    made_service_client, stream_ceremony_response, CancelCeremonyRequest, PauseCeremonyRequest,
    ResumeCeremonyRequest, StartCeremonyRequest, StreamCeremonyRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use serde_json::{json, Value};

const CEREMONY: &str = r#"
version: "1.0"
name: progress_session
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

fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

async fn call(server: &MadeMcpServer, id: u64, tool: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    });
    let line = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

fn assert_ok(answer: &Value) {
    assert!(answer.get("error").is_none(), "{answer:#}");
    assert_ne!(answer["result"]["isError"], true, "{answer:#}");
}

async fn grpc_start(client: &mut MadeServiceClient<tonic::transport::Channel>, id: &str) {
    client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: id.to_owned(),
            definition_yaml: CEREMONY.to_owned(),
            context: None,
            actor_id: "progress-test".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn grpc_delivers_a_new_record_as_a_stream_frame_before_end() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    grpc_start(&mut client, "live-progress").await;

    let mut progress = client
        .stream_ceremony(StreamCeremonyRequest {
            ceremony_id: "live-progress".to_owned(),
            after_sequence: 1,
            max_events: 1,
            wait_timeout_ms: Some(30_000),
        })
        .await
        .unwrap()
        .into_inner();
    let mut writer = made_service_client::MadeServiceClient::new(fixture.channel);
    writer
        .claim_ceremony_step(made_proto::v1::ClaimCeremonyStepRequest {
            ceremony_id: "live-progress".to_owned(),
            step_id: "work".to_owned(),
            actor_kind: "agent".to_owned(),
            lease_owner_id: "progress-host".to_owned(),
            idempotency_key: "progress-claim".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .unwrap();

    let first = progress.message().await.unwrap().unwrap();
    let Some(stream_ceremony_response::Frame::Record(record)) = first.frame else {
        panic!("the first frame after the append must be its record");
    };
    assert_eq!(record.sequence, 2);
    let end = progress.message().await.unwrap().unwrap();
    assert!(matches!(
        end.frame,
        Some(stream_ceremony_response::Frame::End(_))
    ));
}

#[tokio::test]
async fn grpc_stream_pages_lifecycle_events_with_an_exact_resume_cursor() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "lifecycle-progress";
    grpc_start(&mut client, ceremony_id).await;
    client
        .pause_ceremony(PauseCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
            reason: "maintenance".to_owned(),
        })
        .await
        .unwrap();
    client
        .resume_ceremony(ResumeCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .unwrap();
    client
        .cancel_ceremony(CancelCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
            reason: "superseded".to_owned(),
        })
        .await
        .unwrap();

    let mut stream = client
        .stream_ceremony(StreamCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            after_sequence: 1,
            max_events: 3,
            wait_timeout_ms: Some(0),
        })
        .await
        .unwrap()
        .into_inner();
    let mut event_types = Vec::new();
    let resume_after = loop {
        let frame = stream.message().await.unwrap().unwrap();
        match frame.frame.unwrap() {
            stream_ceremony_response::Frame::Record(record) => event_types.push(record.event_type),
            stream_ceremony_response::Frame::End(end) => break end.resume_after_sequence,
        }
    };
    assert_eq!(
        event_types,
        ["ceremony_paused", "ceremony_resumed", "ceremony_cancelled"]
    );
    assert_eq!(resume_after, 4);

    let mut resumed = client
        .stream_ceremony(StreamCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            after_sequence: resume_after,
            max_events: 3,
            wait_timeout_ms: Some(0),
        })
        .await
        .unwrap()
        .into_inner();
    let end = resumed.message().await.unwrap().unwrap();
    let Some(stream_ceremony_response::Frame::End(end)) = end.frame else {
        panic!("the exact cursor must not replay a lifecycle record");
    };
    assert_eq!(end.resume_after_sequence, resume_after);
}

#[tokio::test]
async fn sqlite_reopen_resumes_and_catches_up_with_an_external_writer() {
    let directory = scratch();
    let path = directory.path().join("progress.sqlite3");
    let starter = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(
        EmbeddedMade::open(&path).unwrap(),
    ));
    assert_ok(
        &call(
            &starter,
            1,
            "made_publish_ceremony_definition",
            json!({"definition_yaml": CEREMONY}),
        )
        .await,
    );
    assert_ok(
        &call(
            &starter,
            2,
            "made_start_published_ceremony",
            json!({
                "ceremony_id": "durable-progress", "ceremony": "progress_session",
                "version": "1.0", "actor_id": "host", "actor_kind": "service"
            }),
        )
        .await,
    );
    drop(starter);

    let facade = EmbeddedMade::open(&path).unwrap();
    let id = CeremonyId::new("durable-progress").unwrap();
    let mut replay = facade
        .stream_ceremony(StreamCeremonyInput::new(
            id.clone(),
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::new(1).unwrap(),
            CeremonyProgressWait::IMMEDIATE,
        ))
        .await
        .unwrap();
    let first = replay.next().await.unwrap().unwrap();
    let made_app::usecases::CeremonyProgressFrame::Record(first) = first else {
        panic!("initial replay must contain the opening record");
    };
    assert_eq!(first.sequence().value(), 1);
    let resume = replay.next().await.unwrap().unwrap();
    let made_app::usecases::CeremonyProgressFrame::End(resume) = resume else {
        panic!("bounded replay must finish with a cursor");
    };
    assert_eq!(resume.resume_after_sequence(), StreamVersion::new(1));

    let mut following = facade
        .stream_ceremony(StreamCeremonyInput::new(
            id,
            resume.resume_after_sequence(),
            CeremonyEventPageLimit::new(1).unwrap(),
            CeremonyProgressWait::from_millis(2_000).unwrap(),
        ))
        .await
        .unwrap();
    let writer = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(
        EmbeddedMade::open(&path).unwrap(),
    ));
    let claimed = call(
        &writer,
        3,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": "durable-progress", "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "external-host", "idempotency_key": "external-claim",
            "lease_ttl_ms": 60000
        }),
    )
    .await;
    assert_ok(&claimed);

    let caught_up = following.next().await.unwrap().unwrap();
    let made_app::usecases::CeremonyProgressFrame::Record(caught_up) = caught_up else {
        panic!("polling must observe the other process' append");
    };
    assert_eq!(caught_up.sequence().value(), 2);
    let end = following.next().await.unwrap().unwrap();
    let made_app::usecases::CeremonyProgressFrame::End(end) = end else {
        panic!("caught-up page must finish with a cursor");
    };
    assert_eq!(end.resume_after_sequence(), StreamVersion::new(2));

    let collected = call(
        &writer,
        4,
        "made_stream_ceremony",
        json!({
            "ceremony_id": "durable-progress", "after_sequence": 1,
            "max_events": 1, "wait_timeout_ms": 0
        }),
    )
    .await;
    assert_ok(&collected);
    assert_eq!(
        collected["result"]["structuredContent"]["resume_after_sequence"],
        json!(2)
    );
}
