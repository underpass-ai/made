use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::mcp_protocol::ToolError;

use super::super::proto_to_json as p2j;
use super::{bad_request, intervention_delivery_presenter, intervention_delivery_requests};

const TOOLS: [&str; 4] = [
    "made_pull_protocol_agent_interventions",
    "made_acknowledge_protocol_agent_intervention",
    "made_get_protocol_intervention",
    "made_list_protocol_interventions",
];

pub(super) fn handles(name: &str) -> bool {
    TOOLS.contains(&name)
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_pull_protocol_agent_interventions" => {
            let response = client
                .pull_protocol_agent_interventions(
                    intervention_delivery_requests::pull(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(intervention_delivery_presenter::leases(response.items))
        }
        "made_acknowledge_protocol_agent_intervention" => {
            let response = client
                .acknowledge_protocol_agent_intervention(
                    intervention_delivery_requests::acknowledge(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            response
                .instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("missing protocol instance"))
        }
        "made_get_protocol_intervention" => {
            let response = client
                .get_protocol_intervention(
                    intervention_delivery_requests::get(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(intervention_delivery_presenter::one(response.intervention))
        }
        "made_list_protocol_interventions" => {
            let response = client
                .list_protocol_interventions(
                    intervention_delivery_requests::list(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(intervention_delivery_presenter::page(
                response.interventions,
                response.next_cursor,
            ))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown intervention delivery tool `{other}`"
        ))),
    }
}
