use anyhow::{bail, Context, Result};
use made_proto::v1::RunProtocolRequest;
use tracing::info;

const TECHNICAL_DEBATE_PROTOCOL: &str =
    include_str!("../../../../tests/e2e/protocols/technical-debate.yaml");

pub(crate) async fn verify_technical_debate_protocol(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let response = client
        .run_protocol(RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "e2e-technical-debate".to_owned(),
            definition_yaml: TECHNICAL_DEBATE_PROTOCOL.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "proposal".to_owned(),
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
        .context("RunProtocol failed for technical debate protocol")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected technical debate to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "ADJOURNED" {
        bail!(
            "expected final_state ADJOURNED, got {}",
            response.final_state
        );
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed debate steps, got {}",
            response.steps.len()
        );
    }
    for role in ["PROPONENT", "OPPONENT", "REBUTTER", "ARBITER"] {
        if !response
            .steps
            .iter()
            .any(|step| step.role_id == role && step.status == "COMPLETED")
        {
            bail!("technical debate response is missing completed role {role}");
        }
    }

    let diagram = &response.mermaid_sequence;
    for expected in [
        "sequenceDiagram",
        "state_thesis [facilitation_prompt]",
        "raise_objections [challenge_prompt]",
        "answer_objections [progress_prompt]",
        "render_decision [synthesis_prompt]",
        "decision_rendered -> ADJOURNED",
    ] {
        if !diagram.contains(expected) {
            bail!("technical debate diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        protocol_id = response.protocol_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        diagram = %diagram,
        "technical debate protocol executed and rendered"
    );
    Ok(())
}
