//! Tool-name → gRPC RPC dispatch.
//!
//! One entry per MADE RPC. The dispatcher maps JSON arguments through
//! request mappers, calls the generated tonic client, and maps the response.

use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::{json, Value};
use tonic::transport::Channel;

use super::json_to_proto as j2p;
use super::proto_to_json as p2j;
use super::streaming;

mod ceremony_requests;
mod general_requests;
#[cfg(test)]
mod schema_gate;

/// Dispatch one tool call. Returns the **structured content** of the
/// MCP tool result (just the JSON; the caller wraps it in
/// `tool_success_result`).
#[allow(clippy::too_many_lines)] // one arm per tool; splitting fragments the dispatch table
pub(crate) async fn dispatch(
    channel: Channel,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    let mut client = MadeServiceClient::new(channel);

    match name {
        "made_deliberate" => {
            let request = general_requests::build_deliberate_request(arguments)?;
            let response = client
                .deliberate(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::deliberate_response_to_json(response.into_inner()))
        }

        "made_stream_deliberation" => {
            let request = general_requests::build_stream_deliberation_request(arguments)?;
            let response = client
                .stream_deliberation(request)
                .await
                .map_err(|s| status_error(&s))?;
            streaming::collect_stream(response.into_inner()).await
        }

        "made_get_deliberation_result" => {
            let request = general_requests::build_get_deliberation_result_request(arguments)?;
            let response = client
                .get_deliberation_result(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::GetDeliberationResultResponse { found, result } = response.into_inner();
            Ok(json!({
                "found": found,
                "result": result.map_or(Value::Null, p2j::deliberate_response_to_json),
            }))
        }

        "made_orchestrate" => {
            let request = general_requests::build_orchestrate_request(arguments)?;
            let response = client
                .orchestrate(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::orchestrate_response_to_json(response.into_inner()))
        }

        "made_create_council" => {
            let request = general_requests::build_create_council_request(arguments)?;
            let response = client
                .create_council(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::CreateCouncilResponse { council } = response.into_inner();
            Ok(json!({
                "council": council.map_or(Value::Null, p2j::council_summary_to_json),
            }))
        }

        "made_list_councils" => {
            let request = general_requests::build_list_councils_request(arguments);
            let response = client
                .list_councils(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ListCouncilsResponse { councils } = response.into_inner();
            Ok(json!({
                "councils": councils
                    .into_iter()
                    .map(p2j::council_summary_to_json)
                    .collect::<Vec<_>>(),
            }))
        }

        "made_delete_council" => {
            let request = general_requests::build_delete_council_request(arguments)?;
            let response = client
                .delete_council(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::DeleteCouncilResponse { deleted } = response.into_inner();
            Ok(json!({ "deleted": deleted }))
        }

        "made_register_agent" => {
            let request = general_requests::build_register_agent_request(arguments)?;
            let response = client
                .register_agent(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::RegisterAgentResponse { agent_id } = response.into_inner();
            Ok(json!({ "agent_id": agent_id }))
        }

        "made_unregister_agent" => {
            let request = general_requests::build_unregister_agent_request(arguments)?;
            let response = client
                .unregister_agent(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::UnregisterAgentResponse { unregistered } = response.into_inner();
            Ok(json!({ "unregistered": unregistered }))
        }

        "made_process_trigger_event" => {
            let request = general_requests::build_process_trigger_event_request(arguments)?;
            let response = client
                .process_trigger_event(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ProcessTriggerEventResponse { ack } = response.into_inner();
            Ok(json!({
                "ack": ack.as_ref().map_or(Value::Null, p2j::trigger_ack_to_json),
            }))
        }

        "made_run_council_decision" => {
            let request = general_requests::build_run_council_decision_request(arguments)?;
            let response = client
                .run_council_decision(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::run_council_decision_response_to_json(
                response.into_inner(),
            ))
        }

        "made_register_contract" => {
            let request = general_requests::build_register_contract_request(arguments)?;
            let response = client
                .register_contract(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::RegisterContractResponse { contract_id } = response.into_inner();
            Ok(json!({ "contract_id": contract_id }))
        }

        "made_list_contracts" => {
            let response = client
                .list_contracts(pb::ListContractsRequest {})
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ListContractsResponse { contracts } = response.into_inner();
            Ok(json!({
                "contracts": contracts
                    .into_iter()
                    .map(p2j::output_contract_to_json)
                    .collect::<Vec<_>>(),
            }))
        }

        "made_delete_contract" => {
            let request = general_requests::build_delete_contract_request(arguments)?;
            let response = client
                .delete_contract(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::DeleteContractResponse { deleted } = response.into_inner();
            Ok(json!({ "deleted": deleted }))
        }

        "made_run_ceremony" => {
            let request = general_requests::build_run_ceremony_request(arguments)?;
            let response = client
                .run_ceremony(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::run_ceremony_response_to_json(response.into_inner()))
        }

        // The read side of a working session. The response is the
        // same shape the in-process backend renders, which is the
        // whole point: one tool, either backend, one answer.
        "made_get_ceremony_instance" => {
            let obj = j2p::require_object(arguments, "tools/call.arguments")?;
            let request = pb::GetCeremonyInstanceRequest {
                ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
            };
            let response = client
                .get_ceremony_instance(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::GetCeremonyInstanceResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_list_ceremony_instances" => {
            let response = client
                .list_ceremony_instances(pb::ListCeremonyInstancesRequest {})
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ListCeremonyInstancesResponse { instances } = response.into_inner();
            Ok(json!({
                "instances": instances
                    .into_iter()
                    .map(p2j::ceremony_instance_state_to_json)
                    .collect::<Vec<_>>(),
            }))
        }

        // Every move answers with the session, so one converter serves
        // them all — the same shape the in-process backend renders.
        "made_start_ceremony" => {
            let request = ceremony_requests::build_start_ceremony_request(arguments)?;
            let response = client
                .start_ceremony(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::StartCeremonyResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_start_published_ceremony" => {
            let request = ceremony_requests::build_start_published_ceremony_request(arguments)?;
            let response = client
                .start_published_ceremony(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::StartPublishedCeremonyResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_run_ceremony_step" => {
            let request = ceremony_requests::build_run_ceremony_step_request(arguments)?;
            let response = client
                .run_ceremony_step(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::RunCeremonyStepResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_apply_ceremony_transition" => {
            let request = ceremony_requests::build_apply_ceremony_transition_request(arguments)?;
            let response = client
                .apply_ceremony_transition(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ApplyCeremonyTransitionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_approve_ceremony_guard" => {
            let request = ceremony_requests::build_approve_ceremony_guard_request(arguments)?;
            let response = client
                .approve_ceremony_guard(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::ApproveCeremonyGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_defer_ceremony_guard" => {
            let request = ceremony_requests::build_defer_ceremony_guard_request(arguments)?;
            let response = client
                .defer_ceremony_guard(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::DeferCeremonyGuardResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_assert_ceremony_reason" => {
            let request = ceremony_requests::build_assert_ceremony_reason_request(arguments)?;
            let response = client
                .assert_ceremony_reason(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::AssertCeremonyReasonResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_request_ceremony_intervention" => {
            let request =
                ceremony_requests::build_request_ceremony_intervention_request(arguments)?;
            let response = client
                .request_ceremony_intervention(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::RequestCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_respond_to_ceremony_intervention" => {
            let request =
                ceremony_requests::build_respond_to_ceremony_intervention_request(arguments)?;
            let response = client
                .respond_to_ceremony_intervention(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::RespondToCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_close_ceremony_intervention" => {
            let request = ceremony_requests::build_close_ceremony_intervention_request(arguments)?;
            let response = client
                .close_ceremony_intervention(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::CloseCeremonyInterventionResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_collect_ceremony_evidence" => {
            let request = ceremony_requests::build_collect_ceremony_evidence_request(arguments)?;
            let response = client
                .collect_ceremony_evidence(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::CollectCeremonyEvidenceResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        // Authoring. Validate and explain answer about the YAML in the
        // request; publishing is what puts a version in the catalogue.
        "made_validate_ceremony_draft" => {
            let request = pb::ValidateCeremonyDraftRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)?,
            };
            let response = client
                .validate_ceremony_draft(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::validate_ceremony_draft_to_json(response.into_inner()))
        }

        "made_explain_ceremony_draft" => {
            let request = pb::ExplainCeremonyDraftRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)?,
            };
            let response = client
                .explain_ceremony_draft(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::explain_ceremony_draft_to_json(&response.into_inner()))
        }

        "made_publish_ceremony_definition" => {
            let request = pb::PublishCeremonyDefinitionRequest {
                definition_yaml: ceremony_requests::definition_yaml(arguments)?,
            };
            let response = client
                .publish_ceremony_definition(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::publish_ceremony_definition_to_json(
                &response.into_inner(),
            ))
        }

        "made_diff_ceremony_definitions" => {
            let obj = j2p::require_object(arguments, "tools/call.arguments")?;
            let request = pb::DiffCeremonyDefinitionsRequest {
                before: ceremony_requests::definition_ref(obj, "before")?,
                after: ceremony_requests::definition_ref(obj, "after")?,
            };
            let response = client
                .diff_ceremony_definitions(request)
                .await
                .map_err(|s| status_error(&s))?;
            Ok(p2j::diff_ceremony_definitions_to_json(
                response.into_inner(),
            ))
        }

        "made_bind_ceremony_participants" => {
            let obj = j2p::require_object(arguments, "tools/call.arguments")?;
            let seating = obj
                .get("seating")
                .and_then(Value::as_object)
                .ok_or_else(|| "missing required object `seating`".to_owned())?;
            let request = pb::BindCeremonyParticipantsRequest {
                actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
                actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
                ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
                seating: seating
                    .iter()
                    .map(|(role, specialty)| {
                        specialty
                            .as_str()
                            .map(|specialty| (role.clone(), specialty.to_owned()))
                            .ok_or_else(|| format!("`seating.{role}` must be a string"))
                    })
                    .collect::<Result<_, _>>()?,
            };
            let response = client
                .bind_ceremony_participants(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::BindCeremonyParticipantsResponse { instance } = response.into_inner();
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| "made returned no ceremony instance".to_owned())
        }

        "made_get_status" => {
            let request = general_requests::build_get_status_request(arguments);
            let response = client
                .get_status(request)
                .await
                .map_err(|s| status_error(&s))?;
            let pb::GetStatusResponse {
                version,
                uptime_seconds,
                health,
                stats,
            } = response.into_inner();
            Ok(json!({
                "version": version,
                "uptime_seconds": uptime_seconds,
                "health": health,
                "stats": stats.map_or(Value::Null, p2j::statistics_to_json),
            }))
        }

        "made_get_metrics" => {
            let response = client
                .get_metrics(pb::GetMetricsRequest {})
                .await
                .map_err(|s| status_error(&s))?;
            let pb::GetMetricsResponse { stats } = response.into_inner();
            Ok(json!({
                "stats": stats.map_or(Value::Null, p2j::statistics_to_json),
            }))
        }

        other => Err(format!("unknown made MCP tool `{other}`")),
    }
}

fn status_error(status: &tonic::Status) -> String {
    format!("gRPC {}: {}", status.code(), status.message())
}

// ---------------------------------------------------------------------------
// Request builders. Each takes the raw `tools/call.arguments` JSON
// value and produces a typed proto request. Validation errors come
// back as plain strings; tonic gets a fully-formed proto.
// ---------------------------------------------------------------------------

#[cfg(test)]
use ceremony_requests::{
    build_apply_ceremony_transition_request, build_approve_ceremony_guard_request,
    build_assert_ceremony_reason_request, build_close_ceremony_intervention_request,
    build_collect_ceremony_evidence_request, build_defer_ceremony_guard_request,
    build_request_ceremony_intervention_request, build_respond_to_ceremony_intervention_request,
    build_run_ceremony_step_request, build_start_ceremony_request,
    build_start_published_ceremony_request,
};
#[cfg(test)]
use general_requests::{
    build_create_council_request, build_delete_contract_request, build_delete_council_request,
    build_deliberate_request, build_get_deliberation_result_request, build_orchestrate_request,
    build_process_trigger_event_request, build_register_agent_request,
    build_register_contract_request, build_run_ceremony_request,
    build_run_council_decision_request, build_stream_deliberation_request,
    build_unregister_agent_request,
};
