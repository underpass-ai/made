use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::{
    AgentSummary, CreateCouncilRequest, RegisterAgentRequest, RunCeremonyRequest,
};
use choreo_tests_integration::grpc_fixture::GrpcFixture;

const EDITORIAL_MEETING_CEREMONY: &str =
    include_str!("../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

#[tokio::test]
async fn run_ceremony_executes_yaml_and_returns_trace_diagram() {
    let fixture = GrpcFixture::start().await;
    let mut client = ChoreographerServiceClient::new(fixture.channel);
    for specialty in [
        "facilitation_prompt",
        "persona_prompt",
        "challenge_prompt",
        "synthesis_prompt",
    ] {
        seed_single_noop_agent_council(&mut client, specialty).await;
    }

    let response = client
        .run_ceremony(RunCeremonyRequest {
            ceremony_id: "integration-editorial-meeting".to_owned(),
            definition_yaml: EDITORIAL_MEETING_CEREMONY.to_owned(),
            context: None,
            lease_owner_id: "integration-test".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .expect("RunCeremony should succeed")
        .into_inner();

    assert_eq!(response.ceremony_id, "integration-editorial-meeting");
    assert_eq!(response.definition_name, "editorial_planning_meeting");
    assert_eq!(response.definition_version, "1.0");
    assert_eq!(response.final_state, "CLOSED");
    assert!(response.completed);
    assert_eq!(response.steps.len(), 4);
    assert!(response.steps.iter().all(|step| step.status == "COMPLETED"));
    assert!(response
        .steps
        .iter()
        .any(|step| step.role_id == "FACILITATOR"));
    assert!(response
        .steps
        .iter()
        .any(|step| step.role_id == "CUSTOMER_ADVOCATE"));
    assert!(response
        .steps
        .iter()
        .any(|step| step.role_id == "RISK_REVIEWER"));
    assert!(response
        .steps
        .iter()
        .any(|step| step.role_id == "SYNTHESIZER"));
    assert!(response.mermaid_sequence.contains("sequenceDiagram"));
    assert!(response
        .mermaid_sequence
        .contains("decision_summary [synthesis_prompt]"));
}

async fn seed_single_noop_agent_council(
    client: &mut ChoreographerServiceClient<tonic::transport::Channel>,
    specialty: &str,
) {
    let agent_id = format!("agent-{specialty}-0");
    client
        .register_agent(RegisterAgentRequest {
            specialty: specialty.to_owned(),
            agent: Some(AgentSummary {
                agent_id,
                specialty: specialty.to_owned(),
                kind: "noop".to_owned(),
                attributes: None,
            }),
            agent_config: None,
        })
        .await
        .expect("RegisterAgent should seed ceremony agent");
    client
        .create_council(CreateCouncilRequest {
            specialty: specialty.to_owned(),
            num_agents: 1,
            agent_config: None,
        })
        .await
        .expect("CreateCouncil should seed ceremony council");
}
