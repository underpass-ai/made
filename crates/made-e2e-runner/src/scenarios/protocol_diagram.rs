use anyhow::{bail, Context, Result};
use made_proto::v1::RunProtocolRequest;
use tracing::info;

const EDITORIAL_MEETING_PROTOCOL: &str =
    include_str!("../../../../tests/e2e/protocols/editorial-planning-meeting.yaml");

pub(crate) async fn verify_editorial_meeting_protocol_diagram(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let response = client
        .run_protocol(RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "e2e-editorial-planning-meeting".to_owned(),
            definition_yaml: EDITORIAL_MEETING_PROTOCOL.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "meeting_brief".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue(
                            "E2E protocol fixture".to_owned(),
                        )),
                    },
                )]
                .into_iter()
                .collect(),
            }),
            lease_owner_id: "e2e-runner".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .context("RunProtocol failed for editorial planning protocol")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected protocol to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "CLOSED" {
        bail!("expected final_state CLOSED, got {}", response.final_state);
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed protocol steps, got {}",
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
            bail!("RunProtocol response is missing role {role}");
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
            bail!("protocol diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        protocol_id = response.protocol_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        diagram = %diagram,
        "editorial planning protocol executed and rendered"
    );
    Ok(())
}
