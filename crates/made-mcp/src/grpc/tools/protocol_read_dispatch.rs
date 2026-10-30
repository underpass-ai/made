use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::mcp_protocol::ToolError;

use super::{bad_request, budget_dispatch, j2p, p2j, protocol_agent_status_requests as requests};

pub(super) fn handles(name: &str) -> bool {
    budget_dispatch::handles(name)
        || matches!(
            name,
            "made_get_protocol_instance"
                | "made_list_protocol_instances"
                | "made_search_protocol_instances"
                | "made_list_protocol_agents"
                | "made_get_protocol_agent"
                | "made_report_protocol_agent_status"
        )
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    if budget_dispatch::handles(name) {
        return budget_dispatch::dispatch(client, name, arguments).await;
    }
    if matches!(
        name,
        "made_list_protocol_agents"
            | "made_get_protocol_agent"
            | "made_report_protocol_agent_status"
    ) {
        return dispatch_agent_status(client, name, arguments).await;
    }
    match name {
        "made_get_protocol_instance" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let response = client
                .get_protocol_instance(pb::GetProtocolInstanceRequest {
                    protocol_id: j2p::require_str(obj, "protocol_id")
                        .map_err(bad_request)?
                        .to_owned(),
                })
                .await?
                .into_inner();
            response
                .instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
        }
        "made_list_protocol_instances" => {
            let response = client
                .list_protocol_instances(pb::ListProtocolInstancesRequest {})
                .await?
                .into_inner();
            let entries = response
                .instances
                .into_iter()
                .map(p2j::protocol_instance_listing_entry)
                .collect();
            Ok(crate::renderers::ProtocolInstanceListing::new(entries).to_json())
        }
        "made_search_protocol_instances" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let lifecycle = match j2p::optional_str(obj, "lifecycle") {
                None => pb::ProtocolLifecycleFilter::Unspecified,
                Some("running") => pb::ProtocolLifecycleFilter::Running,
                Some("paused") => pb::ProtocolLifecycleFilter::Paused,
                Some("ended") => pb::ProtocolLifecycleFilter::Ended,
                Some(other) => {
                    return Err(bad_request(format!(
                        "`lifecycle` must be running, paused or ended, got `{other}`"
                    )))
                }
            };
            let response = client
                .search_protocol_instances(pb::SearchProtocolInstancesRequest {
                    cursor: j2p::optional_str(obj, "cursor")
                        .unwrap_or_default()
                        .to_owned(),
                    limit: j2p::optional_u32(obj, "limit").map_err(bad_request)?,
                    id_prefix: j2p::optional_str(obj, "id_prefix")
                        .unwrap_or_default()
                        .to_owned(),
                    lifecycle: lifecycle as i32,
                })
                .await?
                .into_inner();
            let entries = response
                .instances
                .into_iter()
                .map(p2j::protocol_instance_listing_entry)
                .collect();
            Ok(crate::renderers::ProtocolInstanceSearchPage::new(
                entries,
                (!response.next_cursor.is_empty()).then_some(response.next_cursor),
            )
            .to_json())
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown protocol read tool `{other}`"
        ))),
    }
}

async fn dispatch_agent_status(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_list_protocol_agents" => {
            let response = client
                .list_protocol_agents(requests::list(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(serde_json::json!({
                "agents": response.agents.iter().map(p2j::protocol_agent_status_to_json).collect::<Vec<_>>(),
                "next_cursor": response.next_cursor,
            }))
        }
        "made_get_protocol_agent" => client
            .get_protocol_agent(requests::get(arguments).map_err(bad_request)?)
            .await?
            .into_inner()
            .agent
            .as_ref()
            .map(p2j::protocol_agent_status_to_json)
            .ok_or_else(|| ToolError::refused("made returned no protocol agent status")),
        "made_report_protocol_agent_status" => client
            .report_protocol_agent_status(requests::report(arguments).map_err(bad_request)?)
            .await?
            .into_inner()
            .status
            .as_ref()
            .map(p2j::protocol_agent_status_to_json)
            .ok_or_else(|| ToolError::refused("made returned no protocol agent status")),
        _ => unreachable!("agent status dispatch is pre-filtered"),
    }
}
