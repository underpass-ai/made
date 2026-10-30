//! The published catalogue, read over gRPC.

use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::mcp_protocol::{
    ToolError, GET_PROTOCOL_DEFINITION_TOOL, LIST_PROTOCOL_DEFINITIONS_TOOL,
};

use super::definition_catalogue_requests as requests;
use super::{bad_request, p2j};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        LIST_PROTOCOL_DEFINITIONS_TOOL | GET_PROTOCOL_DEFINITION_TOOL
    )
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_list_protocol_definitions" => {
            let response = client
                .list_protocol_definitions(requests::list(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(p2j::list_protocol_definitions_to_json(response))
        }
        "made_get_protocol_definition" => {
            let response = client
                .get_protocol_definition(requests::get(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(p2j::get_protocol_definition_to_json(&response))
        }
        _ => Err(ToolError::invalid_request(format!("unknown tool {name}"))),
    }
}
