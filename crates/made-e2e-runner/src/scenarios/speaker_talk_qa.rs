use anyhow::{bail, Context, Result};
use made_proto::v1::RunProtocolRequest;
use tracing::info;

const SPEAKER_TALK_QA_PROTOCOL: &str =
    include_str!("../../../../tests/e2e/protocols/speaker-talk-qa.yaml");

pub(crate) async fn verify_speaker_talk_qa_protocol(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let response = client
        .run_protocol(RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "e2e-speaker-talk-qa".to_owned(),
            definition_yaml: SPEAKER_TALK_QA_PROTOCOL.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "talk_title".to_owned(),
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
        .context("RunProtocol failed for speaker talk + Q&A protocol")?
        .into_inner();

    if !response.completed {
        bail!(
            "expected speaker talk + Q&A to complete; final_state={}",
            response.final_state
        );
    }
    if response.final_state != "CLOSED" {
        bail!("expected final_state CLOSED, got {}", response.final_state);
    }
    if response.steps.len() != 4 {
        bail!(
            "expected 4 executed talk steps, got {}",
            response.steps.len()
        );
    }
    for role in ["SPEAKER", "AUDIENCE", "SPEAKER_RESPONDENT", "MODERATOR"] {
        if !response
            .steps
            .iter()
            .any(|step| step.role_id == role && step.status == "COMPLETED")
        {
            bail!("speaker talk + Q&A response is missing completed role {role}");
        }
    }

    let diagram = &response.mermaid_sequence;
    for expected in [
        "sequenceDiagram",
        "deliver_talk [facilitation_prompt]",
        "audience_questions [challenge_prompt]",
        "speaker_answers [progress_prompt]",
        "takeaway_synthesis [synthesis_prompt]",
        "takeaways_captured -> CLOSED",
    ] {
        if !diagram.contains(expected) {
            bail!("speaker talk + Q&A diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(
        protocol_id = response.protocol_id,
        final_state = response.final_state,
        steps = response.steps.len(),
        diagram = %diagram,
        "speaker talk + Q&A protocol executed and rendered"
    );
    Ok(())
}
