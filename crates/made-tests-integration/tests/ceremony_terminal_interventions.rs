//! An unresolved intervention blocks completion on every execution surface.

use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_app::usecases::ApplyCeremonyTransitionInput;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{AuditActorKind, CeremonyId, RoleId, TransitionTrigger};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::ApplyCeremonyTransitionRequest;
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::{json, Value};
use tonic::Code;

const CEREMONY: &str = r#"
version: "1.0"
name: pending_question
states:
  - { id: OPEN, initial: true }
  - { id: REVIEW }
  - { id: DONE, terminal: true }
transitions:
  - { from: OPEN, to: REVIEW, trigger: advance }
  - { from: REVIEW, to: DONE, trigger: finish }
roles:
  - id: FACILITATOR
    allowed_actions: [advance, finish, request_intervention]
  - id: OBSERVER
    allowed_actions: [respond_to_intervention]
"#;

const REASON: &str = "ceremony cannot enter a terminal state with open interventions";

async fn call(server: &MadeMcpServer, tool: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    let response: Value = serde_json::from_str(&response).unwrap();
    response["result"].clone()
}

async fn success(server: &MadeMcpServer, tool: &str, arguments: Value) -> Value {
    let response = call(server, tool, arguments).await;
    assert_ne!(response["isError"], true, "{tool}: {response}");
    response["structuredContent"].clone()
}

async fn prepare(server: &MadeMcpServer) {
    success(
        server,
        "made_start_ceremony",
        json!({
            "ceremony_id": "open-question", "actor_id": "operator", "actor_kind": "human",
            "definition_yaml": CEREMONY
        }),
    )
    .await;
    success(
        server,
        "made_request_ceremony_intervention",
        json!({
            "ceremony_id": "open-question", "intervention_id": "question", "role_id": "FACILITATOR",
            "role_kind": "agent", "kind": "opinion", "target_role_ids": ["OBSERVER"],
            "message": "Does the evidence support completion?"
        }),
    )
    .await;
    // Ordinary progress remains legal while the question is open.
    let review = success(
        server,
        "made_apply_ceremony_transition",
        json!({
            "ceremony_id": "open-question", "trigger": "advance", "actor_kind": "agent"
        }),
    )
    .await;
    assert_eq!(review["current_state"], "REVIEW");
    assert_eq!(review["transitions"][0]["enabled"], false);
}

#[tokio::test]
async fn terminal_refusal_preserves_the_stream_and_the_question_on_all_four_surfaces() {
    let remote_store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_ceremony_store(remote_store.clone()))
            .await;
    let grpc_mcp = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    ));
    let local_store = Arc::new(InMemoryCeremonyEventStore::new());
    let facade = EmbeddedMade::builder()
        .with_ceremony_store(local_store.clone())
        .build();
    let embedded_mcp = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(facade.clone()));
    for server in [&grpc_mcp, &embedded_mcp] {
        prepare(server).await;
    }
    let id = CeremonyId::new("open-question").unwrap();
    let heads = (
        remote_store.head(&id).await.unwrap(),
        local_store.head(&id).await.unwrap(),
    );
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    let status = client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: id.as_str().to_owned(),
            actor_kind: "agent".to_owned(),
            trigger: "finish".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::FailedPrecondition);
    assert!(status.message().contains(REASON), "{status}");
    let error = facade
        .apply_transition(ApplyCeremonyTransitionInput::new(
            id.clone(),
            RoleId::new("FACILITATOR").unwrap(),
            AuditActorKind::Agent,
            TransitionTrigger::new("finish").unwrap(),
        ))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), format!("invariant violated: {REASON}"));
    for server in [&grpc_mcp, &embedded_mcp] {
        let refusal = call(
            server,
            "made_apply_ceremony_transition",
            json!({
                "ceremony_id": "open-question", "trigger": "finish", "actor_kind": "agent"
            }),
        )
        .await;
        assert_eq!(refusal["isError"], true);
        assert_eq!(refusal["structuredContent"]["code"], "refused");
        assert_eq!(refusal["structuredContent"]["retryable"], false);
        assert!(refusal["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains(REASON));
    }
    assert_eq!(
        heads,
        (
            remote_store.head(&id).await.unwrap(),
            local_store.head(&id).await.unwrap()
        )
    );
    for server in [&grpc_mcp, &embedded_mcp] {
        // The refused terminal move leaves the human's response writable.
        success(server, "made_respond_to_ceremony_intervention", json!({
            "ceremony_id": "open-question", "intervention_id": "question", "role_id": "OBSERVER",
            "role_kind": "human", "message": "Yes, the evidence supports it."
        })).await;
        // A response does not close the question on behalf of its requester.
        let still_open = call(
            server,
            "made_apply_ceremony_transition",
            json!({
                "ceremony_id": "open-question", "trigger": "finish", "actor_kind": "agent"
            }),
        )
        .await;
        assert_eq!(still_open["isError"], true);
        success(server, "made_close_ceremony_intervention", json!({
            "ceremony_id": "open-question", "intervention_id": "question", "role_id": "FACILITATOR",
            "role_kind": "agent"
        })).await;
        let completed = success(
            server,
            "made_apply_ceremony_transition",
            json!({
                "ceremony_id": "open-question", "trigger": "finish", "actor_kind": "agent"
            }),
        )
        .await;
        assert_eq!(completed["current_state"], "DONE");
        assert_eq!(completed["completed"], true);
    }
}
