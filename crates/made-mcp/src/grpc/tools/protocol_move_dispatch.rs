//! The moves that answer with the whole session.
//!
//! What is left once every family has a dispatcher of its own: the
//! verbs that change a protocol and hand back all of it. One converter
//! serves them because the in-process backend renders the same shape,
//! and keeping them together is what makes that easy to check.

use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::mcp_protocol::ToolError;

use super::super::json_to_proto as j2p;
use super::super::proto_to_json as p2j;
use super::{
    bad_request, design_protocol_request, intervention_request_builders, protocol_history_requests,
    protocol_requests,
};

#[allow(clippy::too_many_lines)] // one arm per move, and the list is the contract
pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        // Every move answers with the session, so one converter serves
        // them all — the same shape the in-process backend renders.
        "made_start_protocol" => {
            let request =
                protocol_requests::build_start_protocol_request(arguments).map_err(bad_request)?;
            let response = client.start_protocol(request).await?;
            let pb::StartProtocolResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_start_published_protocol" => {
            let request = protocol_requests::build_start_published_protocol_request(arguments)
                .map_err(bad_request)?;
            let response = client.start_published_protocol(request).await?;
            let pb::StartPublishedProtocolResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_run_protocol_step" => {
            let request = protocol_requests::build_run_protocol_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.run_protocol_step(request).await?;
            let pb::RunProtocolStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        // Claim, do the work outside the engine, complete. The host
        // performs the step itself; these two calls are how the
        // session learns it was taken on and how it ended.
        "made_claim_protocol_step" => {
            let request = protocol_requests::build_claim_protocol_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.claim_protocol_step(request).await?;
            p2j::protocol_claim_to_json(response.into_inner())
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_complete_protocol_step" => {
            let request = protocol_requests::build_complete_protocol_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.complete_protocol_step(request).await?;
            let pb::CompleteProtocolStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }
        "made_renew_protocol_step_lease" => {
            let request = protocol_requests::build_renew_step_lease_request(arguments)
                .map_err(bad_request)?;
            let receipt = client
                .renew_protocol_step_lease(request)
                .await?
                .into_inner();
            Ok(serde_json::json!({"renewal_id":receipt.renewal_id,
                "claim_fence":receipt.claim_fence,
                "effective_lease_expires_at":receipt.effective_lease_expires_at,
                "renewed_at":receipt.renewed_at}))
        }

        "made_apply_protocol_transition" => {
            let request = protocol_requests::build_apply_protocol_transition_request(arguments)
                .map_err(bad_request)?;
            let response = client.apply_protocol_transition(request).await?;
            let pb::ApplyProtocolTransitionResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_approve_protocol_guard" => {
            let request = protocol_requests::build_approve_protocol_guard_request(arguments)
                .map_err(bad_request)?;
            let response = client.approve_protocol_guard(request).await?;
            let pb::ApproveProtocolGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_defer_protocol_guard" => {
            let request = protocol_requests::build_defer_protocol_guard_request(arguments)
                .map_err(bad_request)?;
            let response = client.defer_protocol_guard(request).await?;
            let pb::DeferProtocolGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_assert_protocol_reason" => {
            let request = protocol_requests::build_assert_protocol_reason_request(arguments)
                .map_err(bad_request)?;
            let response = client.assert_protocol_reason(request).await?;
            let pb::AssertProtocolReasonResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_request_protocol_intervention" => {
            let request =
                intervention_request_builders::build_request_protocol_intervention_request(
                    arguments,
                )
                .map_err(bad_request)?;
            let response = client.request_protocol_intervention(request).await?;
            let pb::RequestProtocolInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_respond_to_protocol_intervention" => {
            let request =
                intervention_request_builders::build_respond_to_protocol_intervention_request(
                    arguments,
                )
                .map_err(bad_request)?;
            let response = client.respond_to_protocol_intervention(request).await?;
            let pb::RespondToProtocolInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_close_protocol_intervention" => {
            let request = protocol_requests::build_close_protocol_intervention_request(arguments)
                .map_err(bad_request)?;
            let response = client.close_protocol_intervention(request).await?;
            let pb::CloseProtocolInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        "made_collect_protocol_evidence" => {
            let request = protocol_requests::build_collect_protocol_evidence_request(arguments)
                .map_err(bad_request)?;
            let response = client.collect_protocol_evidence(request).await?;
            let pb::CollectProtocolEvidenceResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        // Authoring. Validate and explain answer about the YAML in the
        // request; publishing is what puts a version in the catalogue.
        // Designing answers with a document rather than a session:
        // the draft it rendered, already analysed, and what it did
        // with the author's intent.
        "made_design_protocol" => {
            let request = design_protocol_request::build_design_protocol_request(arguments)
                .map_err(bad_request)?;
            let response = client.design_protocol(request).await?;
            Ok(p2j::design_protocol_to_json(response.into_inner()))
        }

        // What the session left behind. Reads, in the order the
        // contract declares them: the stream, the transcript, the
        // report the engine renders from both.
        "made_read_protocol_events" => {
            let request = protocol_history_requests::build_read_protocol_events_request(arguments)
                .map_err(bad_request)?;
            let response = client.read_protocol_events(request).await?;
            Ok(p2j::read_protocol_events_to_json(response.into_inner()))
        }

        "made_stream_protocol" => {
            protocol_history_requests::stream_protocol(client, arguments).await
        }

        "made_pull_protocol_events" => {
            let request = protocol_history_requests::build_pull_protocol_events_request(arguments)
                .map_err(bad_request)?;
            let response = client.pull_protocol_events(request).await?;
            Ok(p2j::pull_protocol_events_to_json(response.into_inner()))
        }

        "made_verify_protocol_journal" => {
            let request =
                protocol_history_requests::build_verify_protocol_journal_request(arguments)
                    .map_err(bad_request)?;
            let response = client.verify_protocol_journal(request).await?;
            Ok(p2j::verify_protocol_journal_to_json(response.into_inner()))
        }

        "made_get_protocol_transcript" => {
            let request =
                protocol_history_requests::build_get_protocol_transcript_request(arguments)
                    .map_err(bad_request)?;
            let response = client.get_protocol_transcript(request).await?;
            Ok(p2j::protocol_transcript_to_json(response.into_inner()))
        }

        "made_generate_protocol_report" => {
            let request =
                protocol_history_requests::build_generate_protocol_report_request(arguments)
                    .map_err(bad_request)?;
            let response = client.generate_protocol_report(request).await?;
            Ok(p2j::protocol_report_to_json(response.into_inner()))
        }

        "made_validate_protocol_draft" => {
            let request = pb::ValidateProtocolDraftRequest {
                definition_yaml: protocol_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.validate_protocol_draft(request).await?;
            Ok(p2j::validate_protocol_draft_to_json(response.into_inner()))
        }

        "made_explain_protocol_draft" => {
            let request = pb::ExplainProtocolDraftRequest {
                definition_yaml: protocol_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.explain_protocol_draft(request).await?;
            Ok(p2j::explain_protocol_draft_to_json(&response.into_inner()))
        }

        "made_publish_protocol_definition" => {
            let request = pb::PublishProtocolDefinitionRequest {
                definition_yaml: protocol_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.publish_protocol_definition(request).await?;
            Ok(p2j::publish_protocol_definition_to_json(
                &response.into_inner(),
            ))
        }

        "made_diff_protocol_definitions" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let request = pb::DiffProtocolDefinitionsRequest {
                before: protocol_requests::definition_ref(obj, "before").map_err(bad_request)?,
                after: protocol_requests::definition_ref(obj, "after").map_err(bad_request)?,
            };
            let response = client.diff_protocol_definitions(request).await?;
            Ok(p2j::diff_protocol_definitions_to_json(
                response.into_inner(),
            ))
        }

        "made_bind_protocol_participants" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let seating = obj
                .get("seating")
                .and_then(Value::as_object)
                .ok_or_else(|| ToolError::invalid_request("missing required object `seating`"))?;
            let request = pb::BindProtocolParticipantsRequest {
                actor_id: j2p::require_str(obj, "actor_id")
                    .map_err(bad_request)?
                    .to_owned(),
                actor_kind: j2p::require_str(obj, "actor_kind")
                    .map_err(bad_request)?
                    .to_owned(),
                protocol_id: j2p::require_str(obj, "protocol_id")
                    .map_err(bad_request)?
                    .to_owned(),
                seating: seating
                    .iter()
                    .map(|(role, specialty)| {
                        specialty
                            .as_str()
                            .map(|specialty| (role.clone(), specialty.to_owned()))
                            .ok_or_else(|| format!("`seating.{role}` must be a string"))
                    })
                    .collect::<Result<_, String>>()
                    .map_err(bad_request)?,
            };
            let response = client.bind_protocol_participants(request).await?;
            let pb::BindProtocolParticipantsResponse { instance } = response.into_inner();
            instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }

        other => Err(ToolError::invalid_request(format!(
            "unknown made MCP tool `{other}`"
        ))),
    }
}
