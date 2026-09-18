use made_adapters::yaml::CeremonyDefinitionYaml;
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::ValidateCeremonyDraftRequest;
use made_tests_integration::grpc_fixture::GrpcFixture;
use serde_json::{json, Value};

const DEFINITION: &str = r"
version: '1.0'
name: global_completion_warning
states:
  - {id: REVIEW, initial: true, execution: concurrent}
  - {id: SYNTHESIS}
  - {id: DONE, terminal: true}
transitions:
  - {from: REVIEW, to: SYNTHESIS, trigger: reviewed, guards: [global]}
  - {from: SYNTHESIS, to: DONE, trigger: finish, guards: [global]}
guards:
  global: {type: automated, check: all_steps_completed}
steps:
  - {id: a, state: REVIEW, handler: embedded_noop}
  - {id: b, state: REVIEW, handler: embedded_noop}
  - {id: c, state: SYNTHESIS, handler: embedded_noop}
roles:
  - {id: A, allowed_actions: [a]}
  - {id: B, allowed_actions: [b]}
  - {id: DRIVER, allowed_actions: [c, reviewed, finish]}
";

#[tokio::test]
async fn global_completion_warning_reaches_all_authoring_surfaces() {
    let fixture = GrpcFixture::start().await;
    let facade = EmbeddedMade::builder().build();
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    let response = client
        .validate_ceremony_draft(ValidateCeremonyDraftRequest {
            definition_yaml: DEFINITION.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(response.publishable);
    assert_eq!(response.error_count, 0);
    assert_eq!(response.warning_count, 1);
    let expected = &response.findings[0].message;
    assert!(expected.contains("all_steps_completed is global"));
    assert!(expected.contains("step_status"));
    assert_eq!(response.findings[0].severity, "warning");

    for server in [
        MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
            format!("http://{}", fixture.addr),
            MadeMcpGrpcTlsConfig::disabled(),
        )),
        MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(facade.clone())),
    ] {
        let request = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
            "params":{"name":"made_validate_ceremony_draft", "arguments":{"definition_yaml":DEFINITION}}});
        let line = server.handle_json_line(&request.to_string()).await.unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        let analysis = &result["result"]["structuredContent"];
        assert_eq!(analysis["publishable"], true, "{result}");
        assert_eq!(analysis["warning_count"], 1);
        assert_eq!(analysis["findings"][0]["message"], expected.as_str());
        assert_eq!(analysis["findings"][0]["severity"], "warning");
    }

    // The facade consumes typed definitions. Advisory analysis survives that
    // publication boundary; it must never become a publication rejection.
    let definition = CeremonyDefinitionYaml::parse_draft_str(DEFINITION)
        .unwrap()
        .publish()
        .unwrap();
    assert_eq!(
        definition
            .analyze()
            .warnings()
            .next()
            .unwrap()
            .defect()
            .to_string(),
        *expected
    );
    facade.publish_definition(definition).await.unwrap();
    assert_eq!(facade.published_definitions().await.unwrap().len(), 1);
}
