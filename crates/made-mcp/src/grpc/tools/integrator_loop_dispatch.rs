use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, integrator_loop_presenter, integrator_loop_requests};

const TOOLS: [&str; 5] = [
    "made_bind_ceremony_integrator",
    "made_get_ceremony_integrator_binding",
    "made_await_integrator_attention",
    "made_acknowledge_integrator_attention",
    "made_list_attention_deliveries",
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
        "made_bind_ceremony_integrator" => {
            let response = client
                .bind_ceremony_integrator(
                    integrator_loop_requests::bind(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(integrator_loop_presenter::bound(response))
        }
        "made_get_ceremony_integrator_binding" => {
            let response = client
                .get_ceremony_integrator_binding(
                    integrator_loop_requests::binding(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(integrator_loop_presenter::one_binding(response.binding))
        }
        "made_await_integrator_attention" => {
            let response = client
                .await_integrator_attention(
                    integrator_loop_requests::await_attention(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(integrator_loop_presenter::batch(response))
        }
        "made_acknowledge_integrator_attention" => {
            let response = client
                .acknowledge_integrator_attention(
                    integrator_loop_requests::acknowledge(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(integrator_loop_presenter::acknowledged(response))
        }
        "made_list_attention_deliveries" => {
            let response = client
                .list_attention_deliveries(
                    integrator_loop_requests::deliveries(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(integrator_loop_presenter::page(
                response.deliveries,
                response.next_cursor,
            ))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown integrator loop tool `{other}`"
        ))),
    }
}
