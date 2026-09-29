//! The published catalogue, read over gRPC.

use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::{ToolError, GET_CEREMONY_DEFINITION_TOOL, LIST_CEREMONY_DEFINITIONS_TOOL};

use super::definition_catalogue_requests as requests;
use super::{bad_request, p2j};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        LIST_CEREMONY_DEFINITIONS_TOOL | GET_CEREMONY_DEFINITION_TOOL
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
        "made_list_ceremony_definitions" => {
            let response = client
                .list_ceremony_definitions(requests::list(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(p2j::list_ceremony_definitions_to_json(response))
        }
        "made_get_ceremony_definition" => {
            let response = client
                .get_ceremony_definition(requests::get(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(p2j::get_ceremony_definition_to_json(&response))
        }
        _ => Err(ToolError::invalid_request(format!("unknown tool {name}"))),
    }
}
