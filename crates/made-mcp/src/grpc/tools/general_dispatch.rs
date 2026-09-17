use super::{
    bad_request, general_requests, json, p2j, pb, streaming, Channel, MadeServiceClient, ToolError,
    Value,
};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_deliberate"
            | "made_stream_deliberation"
            | "made_get_deliberation_result"
            | "made_orchestrate"
            | "made_create_council"
            | "made_list_councils"
            | "made_delete_council"
            | "made_register_agent"
            | "made_unregister_agent"
            | "made_process_trigger_event"
            | "made_run_council_decision"
            | "made_register_contract"
            | "made_list_contracts"
            | "made_delete_contract"
            | "made_run_ceremony"
            | "made_get_status"
            | "made_get_metrics"
    )
}

#[allow(clippy::too_many_lines)] // One auditable match arm per general gRPC tool.
pub(super) async fn dispatch<I>(
    client: &mut MadeServiceClient<tonic::service::interceptor::InterceptedService<Channel, I>>,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError>
where
    I: tonic::service::Interceptor + Clone + Send + Sync + 'static,
{
    match name {
        "made_deliberate" => {
            let request =
                general_requests::build_deliberate_request(arguments).map_err(bad_request)?;
            let response = client.deliberate(request).await?;
            Ok(p2j::deliberate_response_to_json(response.into_inner()))
        }

        "made_stream_deliberation" => {
            let request = general_requests::build_stream_deliberation_request(arguments)
                .map_err(bad_request)?;
            let response = client.stream_deliberation(request).await?;
            streaming::collect_stream(response.into_inner())
                .await
                .map_err(ToolError::refused)
        }

        "made_get_deliberation_result" => {
            let request = general_requests::build_get_deliberation_result_request(arguments)
                .map_err(bad_request)?;
            let response = client.get_deliberation_result(request).await?;
            let pb::GetDeliberationResultResponse { found, result } = response.into_inner();
            Ok(json!({
                "found": found,
                "result": result.map_or(Value::Null, p2j::deliberate_response_to_json),
            }))
        }

        "made_orchestrate" => {
            let request =
                general_requests::build_orchestrate_request(arguments).map_err(bad_request)?;
            let response = client.orchestrate(request).await?;
            Ok(p2j::orchestrate_response_to_json(response.into_inner()))
        }

        "made_create_council" => {
            let request =
                general_requests::build_create_council_request(arguments).map_err(bad_request)?;
            let response = client.create_council(request).await?;
            let pb::CreateCouncilResponse { council } = response.into_inner();
            Ok(json!({
                "council": council.map_or(Value::Null, p2j::council_summary_to_json),
            }))
        }

        "made_list_councils" => {
            let request = general_requests::build_list_councils_request(arguments);
            let response = client.list_councils(request).await?;
            let pb::ListCouncilsResponse { councils } = response.into_inner();
            Ok(json!({
                "councils": councils
                    .into_iter()
                    .map(p2j::council_summary_to_json)
                    .collect::<Vec<_>>(),
            }))
        }

        "made_delete_council" => {
            let request =
                general_requests::build_delete_council_request(arguments).map_err(bad_request)?;
            let response = client.delete_council(request).await?;
            let pb::DeleteCouncilResponse { deleted } = response.into_inner();
            Ok(json!({ "deleted": deleted }))
        }

        "made_register_agent" => {
            let request =
                general_requests::build_register_agent_request(arguments).map_err(bad_request)?;
            let response = client.register_agent(request).await?;
            let pb::RegisterAgentResponse { agent_id } = response.into_inner();
            Ok(json!({ "agent_id": agent_id }))
        }

        "made_unregister_agent" => {
            let request =
                general_requests::build_unregister_agent_request(arguments).map_err(bad_request)?;
            let response = client.unregister_agent(request).await?;
            let pb::UnregisterAgentResponse { unregistered } = response.into_inner();
            Ok(json!({ "unregistered": unregistered }))
        }

        "made_process_trigger_event" => {
            let request = general_requests::build_process_trigger_event_request(arguments)
                .map_err(bad_request)?;
            let response = client.process_trigger_event(request).await?;
            let pb::ProcessTriggerEventResponse { ack } = response.into_inner();
            Ok(json!({
                "ack": ack.as_ref().map_or(Value::Null, p2j::trigger_ack_to_json),
            }))
        }

        "made_run_council_decision" => {
            let request = general_requests::build_run_council_decision_request(arguments)
                .map_err(bad_request)?;
            let response = client.run_council_decision(request).await?;
            Ok(p2j::run_council_decision_response_to_json(
                response.into_inner(),
            ))
        }

        "made_register_contract" => {
            let request = general_requests::build_register_contract_request(arguments)
                .map_err(bad_request)?;
            let response = client.register_contract(request).await?;
            let pb::RegisterContractResponse { contract_id } = response.into_inner();
            Ok(json!({ "contract_id": contract_id }))
        }

        "made_list_contracts" => {
            let response = client.list_contracts(pb::ListContractsRequest {}).await?;
            let pb::ListContractsResponse { contracts } = response.into_inner();
            Ok(json!({
                "contracts": contracts
                    .into_iter()
                    .map(p2j::output_contract_to_json)
                    .collect::<Vec<_>>(),
            }))
        }

        "made_delete_contract" => {
            let request =
                general_requests::build_delete_contract_request(arguments).map_err(bad_request)?;
            let response = client.delete_contract(request).await?;
            let pb::DeleteContractResponse { deleted } = response.into_inner();
            Ok(json!({ "deleted": deleted }))
        }

        "made_run_ceremony" => {
            let request =
                general_requests::build_run_ceremony_request(arguments).map_err(bad_request)?;
            let response = client.run_ceremony(request).await?;
            Ok(p2j::run_ceremony_response_to_json(response.into_inner()))
        }

        "made_get_status" => {
            let request = general_requests::build_get_status_request(arguments);
            let response = client.get_status(request).await?;
            let pb::GetStatusResponse {
                version,
                uptime_seconds,
                health,
                stats,
            } = response.into_inner();
            let statistics = stats.map(p2j::statistics_view);
            Ok(json!({
                "version": version,
                "uptime_seconds": uptime_seconds,
                "health": health,
                "stats": statistics.as_ref().map_or(Value::Null, crate::renderers::StatisticsView::to_json),
            }))
        }

        "made_get_metrics" => {
            let response = client.get_metrics(pb::GetMetricsRequest {}).await?;
            let pb::GetMetricsResponse { stats, .. } = response.into_inner();
            let statistics = stats.map(p2j::statistics_view);
            Ok(crate::renderers::StatisticsView::envelope(
                statistics.as_ref(),
            ))
        }

        _ => unreachable!("general dispatch called for unsupported tool"),
    }
}
