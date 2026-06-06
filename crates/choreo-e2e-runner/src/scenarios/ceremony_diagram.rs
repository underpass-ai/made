use anyhow::{bail, Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::{
    AgentSummary, CreateCouncilRequest, RegisterAgentRequest, RunCeremonyRequest,
};
use tonic::transport::Channel;
use tonic::Code;
use tracing::info;

const EDITORIAL_MEETING_CEREMONY: &str =
    include_str!("../../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

pub(crate) async fn verify_editorial_meeting_ceremony_diagram(
    client: &mut ChoreographerServiceClient<Channel>,
) -> Result<()> {
    for specialty in [
        "facilitation_prompt",
        "persona_prompt",
        "challenge_prompt",
        "synthesis_prompt",
    ] {
        seed_single_noop_agent_council(client, specialty).await?;
    }

    let response = client
        .run_ceremony(RunCeremonyRequest {
            ceremony_id: "e2e-editorial-planning-meeting".to_owned(),
            definition_yaml: EDITORIAL_MEETING_CEREMONY.to_owned(),
            context: None,
            lease_owner_id: "e2e-runner".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .context("RunCeremony failed for editorial planning ceremony")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected ceremony to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "CLOSED" {
        bail!("expected final_state CLOSED, got {}", response.final_state);
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed ceremony steps, got {}",
            response.steps.len()
        );
    }
    for role in [
        "FACILITATOR",
        "CUSTOMER_ADVOCATE",
        "RISK_REVIEWER",
        "SYNTHESIZER",
    ] {
        if !response.steps.iter().any(|step| step.role_id == role) {
            bail!("RunCeremony response is missing role {role}");
        }
    }

    let diagram = &response.mermaid_sequence;
    for expected in [
        "sequenceDiagram",
        "open_room [facilitation_prompt]",
        "customer_story [persona_prompt]",
        "risk_check [challenge_prompt]",
        "decision_summary [synthesis_prompt]",
        "decision_written -> CLOSED",
    ] {
        if !diagram.contains(expected) {
            bail!("ceremony diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        ceremony_id = response.ceremony_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        diagram = %diagram,
        "editorial planning ceremony executed and rendered"
    );
    Ok(())
}

async fn seed_single_noop_agent_council(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    let agent_id = format!("agent-{specialty}-0");
    let register_agent = client
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
        .await;
    match register_agent {
        Ok(_) => {}
        Err(status) if status.code() == Code::AlreadyExists => {}
        Err(status) => return Err(status).context("RegisterAgent failed for ceremony step"),
    }

    let create_council = client
        .create_council(CreateCouncilRequest {
            specialty: specialty.to_owned(),
            num_agents: 1,
            agent_config: None,
        })
        .await;
    match create_council {
        Ok(_) => Ok(()),
        Err(status) if status.code() == Code::AlreadyExists => Ok(()),
        Err(status) => Err(status).context("CreateCouncil failed for ceremony step"),
    }
}
