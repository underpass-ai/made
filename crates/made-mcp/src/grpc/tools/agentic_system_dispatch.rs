use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::bad_request;
use super::{agentic_system_presenter as presenter, agentic_system_requests as requests};

const TOOLS: [&str; 9] = [
    "made_design_agentic_system",
    "made_get_agentic_system",
    "made_list_agentic_systems",
    "made_validate_agentic_system",
    "made_publish_agentic_system",
    "made_instantiate_agentic_system",
    "made_advance_agentic_system_execution",
    "made_get_agentic_system_execution",
    "made_render_agentic_system_diagram",
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
        "made_design_agentic_system" => {
            let response = client
                .design_agentic_system(requests::design(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            presenter::system(response.system)
        }
        "made_get_agentic_system" => {
            let response = client
                .get_agentic_system(requests::get(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            presenter::system(response.system)
        }
        "made_list_agentic_systems" => {
            let response = client
                .list_agentic_systems(requests::list(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            presenter::page(response)
        }
        "made_validate_agentic_system" => {
            let response = client
                .validate_agentic_system(requests::validate(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(presenter::validation(response))
        }
        "made_publish_agentic_system" => {
            let response = client
                .publish_agentic_system(requests::publish(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(presenter::publication(response))
        }
        "made_instantiate_agentic_system" => {
            let response = client
                .instantiate_agentic_system(requests::instantiate(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            presenter::execution(response.execution)
        }
        "made_advance_agentic_system_execution" => {
            let response = client
                .advance_agentic_system_execution(
                    requests::advance(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            presenter::execution(response.execution)
        }
        "made_get_agentic_system_execution" => {
            let response = client
                .get_agentic_system_execution(
                    requests::get_execution(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            presenter::execution(response.execution)
        }
        "made_render_agentic_system_diagram" => {
            let response = client
                .render_agentic_system_diagram(requests::diagram(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(presenter::diagram(&response))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown agentic system tool `{other}`"
        ))),
    }
}
