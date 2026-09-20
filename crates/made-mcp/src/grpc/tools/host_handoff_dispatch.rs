use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, host_handoff_presenter, host_handoff_requests};

const TOOLS: [&str; 2] = [
    "made_record_ceremony_host_handoff",
    "made_inspect_ceremony_resume",
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
        "made_record_ceremony_host_handoff" => {
            let response = client
                .record_ceremony_host_handoff(
                    host_handoff_requests::record(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            response
                .recorded
                .map(host_handoff_presenter::recorded)
                .ok_or_else(|| ToolError::refused("missing host handoff"))
        }
        "made_inspect_ceremony_resume" => {
            let response = client
                .inspect_ceremony_resume(
                    host_handoff_requests::inspect(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            response
                .report
                .map(host_handoff_presenter::preflight)
                .ok_or_else(|| ToolError::refused("missing preflight report"))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown host handoff tool `{other}`"
        ))),
    }
}
