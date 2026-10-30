use anyhow::{bail, Context, Result};
use made_proto::v1::RunProtocolRequest;
use tracing::info;

use super::protocol_vllm_definition::ProtocolVllmDefinition;
use super::protocol_vllm_provider_config::ProtocolVllmProviderConfig;

pub(crate) async fn verify_editorial_meeting_protocol_against_vllm_kind(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let provider = ProtocolVllmProviderConfig::from_env()?;
    let definition = ProtocolVllmDefinition::from_provider_config(&provider)?;

    let response = client
        .run_protocol(RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "e2e-editorial-planning-meeting-vllm".to_owned(),
            definition_yaml: definition.as_yaml().to_owned(),
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
        .context("RunProtocol failed for vLLM editorial planning protocol")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected vLLM protocol to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "CLOSED" {
        bail!(
            "expected vLLM protocol final_state CLOSED, got {}",
            response.final_state
        );
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed vLLM protocol steps, got {}",
            response.steps.len()
        );
    }
    for role in [
        "FACILITATOR",
        "CUSTOMER_ADVOCATE",
        "RISK_REVIEWER",
        "SYNTHESIZER",
    ] {
        if !response
            .steps
            .iter()
            .any(|step| step.role_id == role && step.status == "COMPLETED")
        {
            bail!("vLLM RunProtocol response is missing completed role {role}");
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
            bail!("vLLM protocol diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        protocol_id = response.protocol_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        endpoint = provider.endpoint(),
        model = provider.model(),
        max_tokens = provider.max_tokens(),
        timeout_secs = provider.timeout_secs(),
        diagram = %diagram,
        "vLLM editorial planning protocol executed and rendered"
    );
    Ok(())
}
