//! Public history must retain every sealed authorization field, including nanos.
use std::sync::Arc;

use made_adapters::memory::InMemoryProtocolEventStore;
use made_core::entities::AuditRecord;
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::{ProtocolEventPageLimit, ProtocolId, StreamVersion};
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{ReadProtocolEventsRequest, StartProtocolRequest};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::ParityClock;
use serde_json::{json, Value};
use time::macros::datetime;

const PROTOCOL: &str = r#"
version: "1.0"
name: public_audit
states:
  - {id: WORK, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: WORK, to: DONE, trigger: finish}
roles:
  - {id: DRIVER, allowed_actions: [finish]}
"#;

#[tokio::test]
async fn public_rpc_and_mcp_history_reconstruct_the_original_authorized_record() {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_protocol_store(store.clone())
            .with_clock(Arc::new(ParityClock::at(
                datetime!(2026-09-19 04:00:00.123456789 UTC),
            ))),
    )
    .await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    client
        .start_protocol(StartProtocolRequest {
            protocol_id: "public-authorization".to_owned(),
            actor_id: "payload-actor".to_owned(),
            actor_kind: "human".to_owned(),
            definition_yaml: PROTOCOL.to_owned(),
            context: None,
        })
        .await
        .unwrap();
    let records = store
        .read(
            &ProtocolId::new("public-authorization").unwrap(),
            StreamVersion::EMPTY,
            ProtocolEventPageLimit::new(20).unwrap(),
        )
        .await
        .unwrap();
    let original = &records[0];
    assert_eq!(original.schema_version(), 3);
    let sealed = serde_json::to_value(original.authorization_evidence().unwrap()).unwrap();
    let rpc = client
        .read_protocol_events(ReadProtocolEventsRequest {
            protocol_id: "public-authorization".to_owned(),
            from_version: 0,
            limit: 20,
        })
        .await
        .unwrap()
        .into_inner();
    let public_evidence = rpc.records[0].authorization.as_ref().unwrap();
    assert_eq!(public_evidence.decision_id, sealed["decision_id"]);
    assert_eq!(public_evidence.request_id, sealed["request_id"]);
    assert_eq!(public_evidence.principal_id, "grpc-fixture-host");
    assert_eq!(public_evidence.action, "start_protocol");
    assert_eq!(
        public_evidence
            .scope
            .as_ref()
            .unwrap()
            .protocol_id
            .as_deref(),
        Some("public-authorization")
    );
    assert_eq!(public_evidence.target_digest, sealed["target_digest"]);
    assert_eq!(
        public_evidence.policy_version,
        sealed["policy_version"].as_u64().unwrap()
    );
    assert_eq!(
        public_evidence.admitted_at,
        "2026-09-19T04:00:00.123456789Z"
    );
    assert_eq!(public_evidence.valid_until, sealed["valid_until"]);
    let server = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    ));
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"made_read_protocol_events","arguments":{"protocol_id":"public-authorization"}}});
    let response: Value =
        serde_json::from_str(&server.handle_json_line(&request.to_string()).await.unwrap())
            .unwrap();
    assert_eq!(response["result"]["isError"], false, "{response}");
    let mut public = response["result"]["structuredContent"]["records"][0].clone();
    let object = public.as_object_mut().unwrap();
    let authorization = object
        .remove("authorization")
        .expect("sealed admission is public");
    assert_eq!(authorization, sealed);
    assert_eq!(object.remove("schema_version"), Some(json!(3)));
    // API record fields are flat. Storage uses the explicit v3 envelope so a
    // historical v0.6 reader cannot silently accept unsupported admission data.
    let mut envelope = json!({"schema_version":3,"record":public,"authorization":authorization});
    let rebuilt: AuditRecord = serde_json::from_value(envelope.clone()).unwrap();
    assert_eq!(&rebuilt, original);
    assert!(rebuilt.digest_is_intact().unwrap());
    envelope["authorization"]["principal_id"] = json!("payload-actor");
    let tampered: AuditRecord = serde_json::from_value(envelope).unwrap();
    assert!(
        !tampered.digest_is_intact().unwrap(),
        "principal tampering must break the seal"
    );
}
