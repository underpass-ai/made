use anyhow::{bail, Context, Result};
use made_proto::v1::RunProtocolRequest;
use tracing::info;

const SPRINT_PLANNING_PROTOCOL: &str =
    include_str!("../../../../tests/e2e/protocols/sprint-planning.yaml");

pub(crate) async fn verify_sprint_planning_protocol(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let response = client
        .run_protocol(RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "e2e-sprint-planning".to_owned(),
            definition_yaml: SPRINT_PLANNING_PROTOCOL.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "sprint_goal".to_owned(),
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
        .context("RunProtocol failed for sprint planning protocol")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected sprint planning to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "CLOSED" {
        bail!("expected final_state CLOSED, got {}", response.final_state);
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed planning steps, got {}",
            response.steps.len()
        );
    }
    for role in ["PRODUCT_OWNER", "ENGINEER", "SCRUM_MASTER", "DELIVERY_LEAD"] {
        if !response
            .steps
            .iter()
            .any(|step| step.role_id == role && step.status == "COMPLETED")
        {
            bail!("sprint planning response is missing completed role {role}");
        }
    }

    let diagram = &response.mermaid_sequence;
    for expected in [
        "sequenceDiagram",
        "present_priorities [facilitation_prompt]",
        "task_breakdown [progress_prompt]",
        "capacity_review [challenge_prompt]",
        "scope_commitment [synthesis_prompt]",
        "scope_committed -> CLOSED",
    ] {
        if !diagram.contains(expected) {
            bail!("sprint planning diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        protocol_id = response.protocol_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        diagram = %diagram,
        "sprint planning protocol executed and rendered"
    );
    Ok(())
}
