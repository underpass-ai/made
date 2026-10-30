//! Required inputs are one aggregate invariant for every opening path.
use std::sync::Arc;

use made_adapters::memory::InMemoryProtocolEventStore;
use made_adapters::yaml::ProtocolDefinitionYaml;
use made_app::usecases::{RunProtocolInput, StartProtocolInput};
use made_core::entities::{ProtocolInstance, PublishedProtocolDefinition};
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::{
    Attributes, AuditActorKind, DurationMs, LeaseOwnerId, ProtocolContext, ProtocolId,
    StreamVersion,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    PublishProtocolDefinitionRequest, RunProtocolRequest, StartProtocolRequest,
    StartPublishedProtocolRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::{json, Value};
use tonic::Code;

const PROTOCOL: &str = r#"
version: "1.0"
name: required_inputs
inputs:
  required: [zeta, alpha]
  optional: [extra]
states:
  - { id: OPEN, initial: true }
  - { id: DONE, terminal: true }
transitions:
  - { from: OPEN, to: DONE, trigger: finish }
roles:
  - { id: OPERATOR, allowed_actions: [finish] }
"#;
const REASON: &str = "missing required protocol inputs: alpha, zeta";

fn id(value: &str) -> ProtocolId {
    ProtocolId::new(value).unwrap()
}

fn context(value: Value) -> ProtocolContext {
    ProtocolContext::new(Attributes::new(serde_json::from_value(value).unwrap()).unwrap())
}

async fn call(server: &MadeMcpServer, name: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
        "params":{"name":name,"arguments":arguments}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str::<Value>(&response).unwrap()["result"].clone()
}

#[test]
fn aggregate_openings_refuse_all_missing_names_and_accept_present_nulls() {
    let definition = ProtocolDefinitionYaml::parse_str(PROTOCOL).unwrap();
    let published = PublishedProtocolDefinition::seal(definition.clone()).unwrap();
    let now = time::OffsetDateTime::UNIX_EPOCH;
    assert_eq!(
        ProtocolInstance::decide_start(
            id("missing"),
            &definition,
            ProtocolContext::empty(),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        REASON
    );
    assert_eq!(
        ProtocolInstance::decide_start_bound(
            id("missing"),
            &published,
            ProtocolContext::empty(),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        REASON
    );
    assert_eq!(
        ProtocolInstance::start(id("missing"), &definition, ProtocolContext::empty(), now)
            .unwrap_err()
            .to_string(),
        REASON
    );
    assert_eq!(
        ProtocolInstance::start_bound(id("missing"), &published, ProtocolContext::empty(), now)
            .unwrap_err()
            .to_string(),
        REASON
    );
    assert_eq!(
        ProtocolInstance::decide_start(
            id("partial"),
            &definition,
            context(json!({"zeta":null})),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        "missing required protocol inputs: alpha"
    );
    let supplied = context(json!({"alpha":null,"zeta":false}));
    assert_eq!(
        ProtocolInstance::decide_start(
            id("optional-only"),
            &definition,
            context(json!({"extra":"present"})),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        REASON
    );
    let events =
        ProtocolInstance::decide_start(id("present"), &definition, supplied.clone(), None, now)
            .unwrap();
    assert_eq!(events.len(), 1);
    let instance = ProtocolInstance::rehydrate(events.iter()).unwrap();
    assert_eq!(instance.context(), &supplied);
    assert!(!instance
        .context()
        .attributes()
        .as_map()
        .contains_key("extra"));
}

#[tokio::test]
async fn all_opening_paths_refuse_without_events_through_both_mcp_backends() {
    let remote_store = Arc::new(InMemoryProtocolEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_protocol_store(remote_store.clone()))
            .await;
    let remote = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    ));
    let local_store = Arc::new(InMemoryProtocolEventStore::new());
    let facade = EmbeddedMade::builder()
        .with_protocol_store(local_store.clone())
        .build();
    let local = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(facade));
    for (server, store) in [(&remote, &remote_store), (&local, &local_store)] {
        let published = call(
            server,
            "made_publish_protocol_definition",
            json!({"definition_yaml":PROTOCOL}),
        )
        .await;
        assert_ne!(published["isError"], true, "{published}");
        for tool in [
            "made_start_protocol",
            "made_start_published_protocol",
            "made_run_protocol",
        ] {
            let mut arguments =
                json!({"protocol_id":tool, "actor_id":"operator", "actor_kind":"human"});
            if tool == "made_start_published_protocol" {
                arguments["protocol"] = json!("required_inputs");
                arguments["version"] = json!("1.0");
            } else {
                arguments["definition_yaml"] = json!(PROTOCOL);
            }
            let refusal = call(server, tool, arguments.clone()).await;
            assert_eq!(refusal["isError"], true, "{tool}: {refusal}");
            assert_eq!(refusal["structuredContent"]["message"], REASON);
            assert_eq!(refusal["structuredContent"]["retryable"], false);
            assert_eq!(store.head(&id(tool)).await.unwrap(), StreamVersion::EMPTY);
            // Retrying the same id with the missing keys succeeds: no orphan instance.
            arguments["context"] = json!({"alpha":null,"zeta":false});
            let started = call(server, tool, arguments).await;
            assert_ne!(started["isError"], true, "{tool}: {started}");
            assert_ne!(store.head(&id(tool)).await.unwrap(), StreamVersion::EMPTY);
        }
    }
}

#[tokio::test]
async fn direct_proto_and_facade_share_the_same_refusal_for_every_opening() {
    let remote_store = Arc::new(InMemoryProtocolEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_protocol_store(remote_store.clone()))
            .await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: PROTOCOL.to_owned(),
        })
        .await
        .unwrap();
    let start = client
        .start_protocol(StartProtocolRequest {
            protocol_id: "direct-start".into(),
            definition_yaml: PROTOCOL.into(),
            actor_id: "operator".into(),
            actor_kind: "human".into(),
            context: None,
        })
        .await
        .unwrap_err();
    let published = client
        .start_published_protocol(StartPublishedProtocolRequest {
            protocol_id: "direct-published".into(),
            protocol: "required_inputs".into(),
            version: "1.0".into(),
            actor_id: "operator".into(),
            actor_kind: "human".into(),
            context: None,
            budget_limits: None,
        })
        .await
        .unwrap_err();
    let run = client
        .run_protocol(RunProtocolRequest {
            protocol_id: "direct-run".into(),
            definition_yaml: PROTOCOL.into(),
            actor_id: "operator".into(),
            actor_kind: "human".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    for (status, name) in [
        (start, "direct-start"),
        (published, "direct-published"),
        (run, "direct-run"),
    ] {
        assert_eq!(status.code(), Code::InvalidArgument);
        assert_eq!(status.message(), REASON);
        assert_stream_empty(&remote_store, name).await;
    }
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let facade = EmbeddedMade::builder()
        .with_protocol_store(store.clone())
        .build();
    let definition = ProtocolDefinitionYaml::parse_str(PROTOCOL).unwrap();
    facade.mount_definition(definition.clone()).await.unwrap();
    facade.publish_definition(definition.clone()).await.unwrap();
    let input = |name| {
        StartProtocolInput::new(
            id(name),
            definition.name().clone(),
            definition.version().clone(),
            ProtocolContext::empty(),
            "operator",
            AuditActorKind::Human,
        )
    };
    assert_eq!(
        facade
            .start(input("facade-start"))
            .await
            .unwrap_err()
            .to_string(),
        REASON
    );
    assert_eq!(
        facade
            .start_published(input("facade-published"))
            .await
            .unwrap_err()
            .to_string(),
        REASON
    );
    let run = RunProtocolInput::new(
        id("facade-run"),
        definition,
        ProtocolContext::empty(),
        LeaseOwnerId::new("operator").unwrap(),
        DurationMs::from_millis(60_000),
        "operator",
        AuditActorKind::Human,
    );
    assert_eq!(facade.run(run).await.unwrap_err().to_string(), REASON);
    for name in ["facade-start", "facade-published", "facade-run"] {
        assert_stream_empty(&store, name).await;
    }
}

async fn assert_stream_empty(store: &InMemoryProtocolEventStore, name: &str) {
    assert_eq!(store.head(&id(name)).await.unwrap(), StreamVersion::EMPTY);
}
