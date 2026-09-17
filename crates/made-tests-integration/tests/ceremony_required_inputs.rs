//! Required inputs are one aggregate invariant for every opening path.
use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{RunCeremonyInput, StartCeremonyInput};
use made_core::entities::{CeremonyInstance, PublishedCeremonyDefinition};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, LeaseOwnerId,
    StreamVersion,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    PublishCeremonyDefinitionRequest, RunCeremonyRequest, StartCeremonyRequest,
    StartPublishedCeremonyRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::{json, Value};
use tonic::Code;

const CEREMONY: &str = r#"
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
const REASON: &str = "missing required ceremony inputs: alpha, zeta";

fn id(value: &str) -> CeremonyId {
    CeremonyId::new(value).unwrap()
}

fn context(value: Value) -> CeremonyContext {
    CeremonyContext::new(Attributes::new(serde_json::from_value(value).unwrap()).unwrap())
}

async fn call(server: &MadeMcpServer, name: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
        "params":{"name":name,"arguments":arguments}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str::<Value>(&response).unwrap()["result"].clone()
}

#[test]
fn aggregate_openings_refuse_all_missing_names_and_accept_present_nulls() {
    let definition = CeremonyDefinitionYaml::parse_str(CEREMONY).unwrap();
    let published = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
    let now = time::OffsetDateTime::UNIX_EPOCH;
    assert_eq!(
        CeremonyInstance::decide_start(
            id("missing"),
            &definition,
            CeremonyContext::empty(),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        REASON
    );
    assert_eq!(
        CeremonyInstance::decide_start_bound(
            id("missing"),
            &published,
            CeremonyContext::empty(),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        REASON
    );
    assert_eq!(
        CeremonyInstance::start(id("missing"), &definition, CeremonyContext::empty(), now)
            .unwrap_err()
            .to_string(),
        REASON
    );
    assert_eq!(
        CeremonyInstance::start_bound(id("missing"), &published, CeremonyContext::empty(), now)
            .unwrap_err()
            .to_string(),
        REASON
    );
    assert_eq!(
        CeremonyInstance::decide_start(
            id("partial"),
            &definition,
            context(json!({"zeta":null})),
            None,
            now
        )
        .unwrap_err()
        .to_string(),
        "missing required ceremony inputs: alpha"
    );
    let supplied = context(json!({"alpha":null,"zeta":false}));
    let events =
        CeremonyInstance::decide_start(id("present"), &definition, supplied.clone(), None, now)
            .unwrap();
    assert_eq!(events.len(), 1);
    let instance = CeremonyInstance::rehydrate(events.iter()).unwrap();
    assert_eq!(instance.context(), &supplied);
    assert!(!instance
        .context()
        .attributes()
        .as_map()
        .contains_key("extra"));
}

#[tokio::test]
async fn all_opening_paths_refuse_without_events_through_both_mcp_backends() {
    let remote_store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_ceremony_store(remote_store.clone()))
            .await;
    let remote = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    ));
    let local_store = Arc::new(InMemoryCeremonyEventStore::new());
    let facade = EmbeddedMade::builder()
        .with_ceremony_store(local_store.clone())
        .build();
    let local = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(facade));
    for (server, store) in [(&remote, &remote_store), (&local, &local_store)] {
        let published = call(
            server,
            "made_publish_ceremony_definition",
            json!({"definition_yaml":CEREMONY}),
        )
        .await;
        assert_ne!(published["isError"], true, "{published}");
        for tool in [
            "made_start_ceremony",
            "made_start_published_ceremony",
            "made_run_ceremony",
        ] {
            let mut arguments =
                json!({"ceremony_id":tool, "actor_id":"operator", "actor_kind":"human"});
            if tool == "made_start_published_ceremony" {
                arguments["ceremony"] = json!("required_inputs");
                arguments["version"] = json!("1.0");
            } else {
                arguments["definition_yaml"] = json!(CEREMONY);
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
    let remote_store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_ceremony_store(remote_store.clone()))
            .await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: CEREMONY.to_owned(),
        })
        .await
        .unwrap();
    let start = client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: "direct-start".into(),
            definition_yaml: CEREMONY.into(),
            actor_id: "operator".into(),
            actor_kind: "human".into(),
            context: None,
        })
        .await
        .unwrap_err();
    let published = client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: "direct-published".into(),
            ceremony: "required_inputs".into(),
            version: "1.0".into(),
            actor_id: "operator".into(),
            actor_kind: "human".into(),
            context: None,
        })
        .await
        .unwrap_err();
    let run = client
        .run_ceremony(RunCeremonyRequest {
            ceremony_id: "direct-run".into(),
            definition_yaml: CEREMONY.into(),
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
        assert_eq!(
            remote_store.head(&id(name)).await.unwrap(),
            StreamVersion::EMPTY
        );
    }
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let facade = EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .build();
    let definition = CeremonyDefinitionYaml::parse_str(CEREMONY).unwrap();
    facade.mount_definition(definition.clone()).await.unwrap();
    facade.publish_definition(definition.clone()).await.unwrap();
    let input = |name| {
        StartCeremonyInput::new(
            id(name),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
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
    let run = RunCeremonyInput::new(
        id("facade-run"),
        definition,
        CeremonyContext::empty(),
        LeaseOwnerId::new("operator").unwrap(),
        DurationMs::from_millis(60_000),
        "operator",
        AuditActorKind::Human,
    );
    assert_eq!(facade.run(run).await.unwrap_err().to_string(), REASON);
    for name in ["facade-start", "facade-published", "facade-run"] {
        assert_eq!(store.head(&id(name)).await.unwrap(), StreamVersion::EMPTY);
    }
}
