use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::mcp_protocol::ToolError;

use super::lifecycle_requests::{
    build_cancel_protocol_request, build_enforce_protocol_deadlines_request,
    build_pause_protocol_request, build_resume_protocol_request,
};
use super::{bad_request, p2j};

const TOOLS: [&str; 4] = [
    "made_pause_protocol",
    "made_resume_protocol",
    "made_cancel_protocol",
    "made_enforce_protocol_deadlines",
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
    let instance = match name {
        "made_pause_protocol" => {
            client
                .pause_protocol(build_pause_protocol_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_resume_protocol" => {
            client
                .resume_protocol(build_resume_protocol_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_cancel_protocol" => {
            client
                .cancel_protocol(build_cancel_protocol_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_enforce_protocol_deadlines" => {
            client
                .enforce_protocol_deadlines(
                    build_enforce_protocol_deadlines_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner()
                .instance
        }
        other => {
            return Err(ToolError::invalid_request(format!(
                "unknown lifecycle tool `{other}`"
            )))
        }
    };
    instance
        .map(p2j::protocol_instance_state_to_json)
        .ok_or_else(|| ToolError::refused("made returned no protocol instance"))
}
