//! Tool-name → gRPC RPC dispatch.
//!
//! One entry per MADE RPC. The dispatcher maps JSON arguments through
//! request mappers, calls the generated tonic client, and maps the response.

use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::{json, Value};
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::json_to_proto as j2p;
use super::proto_to_json as p2j;
use super::streaming;

mod ceremony_history_requests;
mod ceremony_requests;
mod design_ceremony_request;
mod general_dispatch;
mod general_requests;

// One rule for the runner an omitted `lease_owner_id` becomes; the
// one-shot run mapper lives in `json_to_proto` and uses the same one.
pub(in crate::grpc) use ceremony_requests::{lease_owner_id, lease_ttl_ms};
#[cfg(test)]
mod schema_gate;

/// Turning the caller's JSON into a proto request.
///
/// Everything the request mappers report is the call's own fault — a
/// field missing, a field of the wrong type, a value outside the enum
/// the contract names — so one function says so instead of each arm
/// choosing a code.
fn bad_request(message: String) -> ToolError {
    ToolError::invalid_request(message)
}

/// Dispatch one tool call. Returns the **structured content** of the
/// MCP tool result (just the JSON; the caller wraps it in
/// `tool_success_result`).
#[allow(clippy::too_many_lines)] // one arm per tool; splitting fragments the dispatch table
pub(crate) async fn dispatch(
    channel: Channel,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let mut client = MadeServiceClient::new(channel);
    if general_dispatch::handles(name) {
        return general_dispatch::dispatch(&mut client, name, arguments).await;
    }

    match name {
        // The read side of a working session. The response is the
        // same shape the in-process backend renders, which is the
        // whole point: one tool, either backend, one answer.
        "made_get_ceremony_instance" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let request = pb::GetCeremonyInstanceRequest {
                ceremony_id: j2p::require_str(obj, "ceremony_id")
                    .map_err(bad_request)?
                    .to_owned(),
            };
            let response = client.get_ceremony_instance(request).await?;
            let pb::GetCeremonyInstanceResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_list_ceremony_instances" => {
            let response = client
                .list_ceremony_instances(pb::ListCeremonyInstancesRequest {})
                .await?;
            let pb::ListCeremonyInstancesResponse { instances } = response.into_inner();
            let entries = instances
                .into_iter()
                .map(p2j::ceremony_instance_listing_entry)
                .collect::<Vec<_>>();
            Ok(crate::renderers::CeremonyInstanceListing::new(entries).to_json())
        }

        // Every move answers with the session, so one converter serves
        // them all — the same shape the in-process backend renders.
        "made_start_ceremony" => {
            let request =
                ceremony_requests::build_start_ceremony_request(arguments).map_err(bad_request)?;
            let response = client.start_ceremony(request).await?;
            let pb::StartCeremonyResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_start_published_ceremony" => {
            let request = ceremony_requests::build_start_published_ceremony_request(arguments)
                .map_err(bad_request)?;
            let response = client.start_published_ceremony(request).await?;
            let pb::StartPublishedCeremonyResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_run_ceremony_step" => {
            let request = ceremony_requests::build_run_ceremony_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.run_ceremony_step(request).await?;
            let pb::RunCeremonyStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        // Claim, do the work outside the engine, complete. The host
        // performs the step itself; these two calls are how the
        // session learns it was taken on and how it ended.
        "made_claim_ceremony_step" => {
            let request = ceremony_requests::build_claim_ceremony_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.claim_ceremony_step(request).await?;
            let pb::ClaimCeremonyStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_complete_ceremony_step" => {
            let request = ceremony_requests::build_complete_ceremony_step_request(arguments)
                .map_err(bad_request)?;
            let response = client.complete_ceremony_step(request).await?;
            let pb::CompleteCeremonyStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_apply_ceremony_transition" => {
            let request = ceremony_requests::build_apply_ceremony_transition_request(arguments)
                .map_err(bad_request)?;
            let response = client.apply_ceremony_transition(request).await?;
            let pb::ApplyCeremonyTransitionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_approve_ceremony_guard" => {
            let request = ceremony_requests::build_approve_ceremony_guard_request(arguments)
                .map_err(bad_request)?;
            let response = client.approve_ceremony_guard(request).await?;
            let pb::ApproveCeremonyGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_defer_ceremony_guard" => {
            let request = ceremony_requests::build_defer_ceremony_guard_request(arguments)
                .map_err(bad_request)?;
            let response = client.defer_ceremony_guard(request).await?;
            let pb::DeferCeremonyGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_assert_ceremony_reason" => {
            let request = ceremony_requests::build_assert_ceremony_reason_request(arguments)
                .map_err(bad_request)?;
            let response = client.assert_ceremony_reason(request).await?;
            let pb::AssertCeremonyReasonResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_request_ceremony_intervention" => {
            let request = ceremony_requests::build_request_ceremony_intervention_request(arguments)
                .map_err(bad_request)?;
            let response = client.request_ceremony_intervention(request).await?;
            let pb::RequestCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_respond_to_ceremony_intervention" => {
            let request =
                ceremony_requests::build_respond_to_ceremony_intervention_request(arguments)
                    .map_err(bad_request)?;
            let response = client.respond_to_ceremony_intervention(request).await?;
            let pb::RespondToCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_close_ceremony_intervention" => {
            let request = ceremony_requests::build_close_ceremony_intervention_request(arguments)
                .map_err(bad_request)?;
            let response = client.close_ceremony_intervention(request).await?;
            let pb::CloseCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        "made_collect_ceremony_evidence" => {
            let request = ceremony_requests::build_collect_ceremony_evidence_request(arguments)
                .map_err(bad_request)?;
            let response = client.collect_ceremony_evidence(request).await?;
            let pb::CollectCeremonyEvidenceResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        // Authoring. Validate and explain answer about the YAML in the
        // request; publishing is what puts a version in the catalogue.
        // Designing answers with a document rather than a session:
        // the draft it rendered, already analysed, and what it did
        // with the author's intent.
        "made_design_ceremony" => {
            let request = design_ceremony_request::build_design_ceremony_request(arguments)
                .map_err(bad_request)?;
            let response = client.design_ceremony(request).await?;
            Ok(p2j::design_ceremony_to_json(response.into_inner()))
        }

        // What the session left behind. Reads, in the order the
        // contract declares them: the stream, the transcript, the
        // report the engine renders from both.
        "made_read_ceremony_events" => {
            let request = ceremony_history_requests::build_read_ceremony_events_request(arguments)
                .map_err(bad_request)?;
            let response = client.read_ceremony_events(request).await?;
            Ok(p2j::read_ceremony_events_to_json(response.into_inner()))
        }

        "made_verify_ceremony_journal" => {
            let request =
                ceremony_history_requests::build_verify_ceremony_journal_request(arguments)
                    .map_err(bad_request)?;
            let response = client.verify_ceremony_journal(request).await?;
            Ok(p2j::verify_ceremony_journal_to_json(response.into_inner()))
        }

        "made_get_ceremony_transcript" => {
            let request =
                ceremony_history_requests::build_get_ceremony_transcript_request(arguments)
                    .map_err(bad_request)?;
            let response = client.get_ceremony_transcript(request).await?;
            Ok(p2j::ceremony_transcript_to_json(response.into_inner()))
        }

        "made_generate_ceremony_report" => {
            let request =
                ceremony_history_requests::build_generate_ceremony_report_request(arguments)
                    .map_err(bad_request)?;
            let response = client.generate_ceremony_report(request).await?;
            Ok(p2j::ceremony_report_to_json(response.into_inner()))
        }

        "made_validate_ceremony_draft" => {
            let request = pb::ValidateCeremonyDraftRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.validate_ceremony_draft(request).await?;
            Ok(p2j::validate_ceremony_draft_to_json(response.into_inner()))
        }

        "made_explain_ceremony_draft" => {
            let request = pb::ExplainCeremonyDraftRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.explain_ceremony_draft(request).await?;
            Ok(p2j::explain_ceremony_draft_to_json(&response.into_inner()))
        }

        "made_publish_ceremony_definition" => {
            let request = pb::PublishCeremonyDefinitionRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)
                    .map_err(bad_request)?,
            };
            let response = client.publish_ceremony_definition(request).await?;
            Ok(p2j::publish_ceremony_definition_to_json(
                &response.into_inner(),
            ))
        }

        "made_diff_ceremony_definitions" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let request = pb::DiffCeremonyDefinitionsRequest {
                before: ceremony_requests::definition_ref(obj, "before").map_err(bad_request)?,
                after: ceremony_requests::definition_ref(obj, "after").map_err(bad_request)?,
            };
            let response = client.diff_ceremony_definitions(request).await?;
            Ok(p2j::diff_ceremony_definitions_to_json(
                response.into_inner(),
            ))
        }

        "made_bind_ceremony_participants" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let seating = obj
                .get("seating")
                .and_then(Value::as_object)
                .ok_or_else(|| ToolError::invalid_request("missing required object `seating`"))?;
            let request = pb::BindCeremonyParticipantsRequest {
                actor_id: j2p::require_str(obj, "actor_id")
                    .map_err(bad_request)?
                    .to_owned(),
                actor_kind: j2p::require_str(obj, "actor_kind")
                    .map_err(bad_request)?
                    .to_owned(),
                ceremony_id: j2p::require_str(obj, "ceremony_id")
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
            let response = client.bind_ceremony_participants(request).await?;
            let pb::BindCeremonyParticipantsResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }

        other => Err(ToolError::invalid_request(format!(
            "unknown made MCP tool `{other}`"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Request builders. Each takes the raw `tools/call.arguments` JSON
// value and produces a typed proto request. Validation errors come
// back as plain strings; tonic gets a fully-formed proto.
// ---------------------------------------------------------------------------

#[cfg(test)]
use ceremony_history_requests::{
    build_generate_ceremony_report_request, build_get_ceremony_transcript_request,
    build_read_ceremony_events_request,
};
#[cfg(test)]
use ceremony_requests::{
    build_apply_ceremony_transition_request, build_approve_ceremony_guard_request,
    build_assert_ceremony_reason_request, build_claim_ceremony_step_request,
    build_close_ceremony_intervention_request, build_collect_ceremony_evidence_request,
    build_complete_ceremony_step_request, build_defer_ceremony_guard_request,
    build_request_ceremony_intervention_request, build_respond_to_ceremony_intervention_request,
    build_run_ceremony_step_request, build_start_ceremony_request,
    build_start_published_ceremony_request,
};
#[cfg(test)]
use design_ceremony_request::build_design_ceremony_request;
#[cfg(test)]
use general_requests::{
    build_create_council_request, build_delete_contract_request, build_delete_council_request,
    build_deliberate_request, build_get_deliberation_result_request, build_orchestrate_request,
    build_process_trigger_event_request, build_register_agent_request,
    build_register_contract_request, build_run_ceremony_request,
    build_run_council_decision_request, build_stream_deliberation_request,
    build_unregister_agent_request,
};
