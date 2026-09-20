use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, succession_presenter, succession_requests};

const TOOLS: [&str; 2] = [
    "made_plan_ceremony_successor",
    "made_start_ceremony_successor",
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
        "made_plan_ceremony_successor" => {
            let response = client
                .plan_ceremony_successor(succession_requests::plan(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            response
                .plan
                .map(succession_presenter::plan_view)
                .ok_or_else(|| ToolError::refused("missing successor plan"))
        }
        "made_start_ceremony_successor" => {
            let response = client
                .start_ceremony_successor(
                    succession_requests::start(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(succession_presenter::started(response))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown succession tool `{other}`"
        ))),
    }
}
