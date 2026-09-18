use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::lifecycle_requests::{
    build_cancel_ceremony_request, build_enforce_ceremony_deadlines_request,
    build_pause_ceremony_request, build_resume_ceremony_request,
};
use super::{bad_request, p2j};

const TOOLS: [&str; 4] = [
    "made_pause_ceremony",
    "made_resume_ceremony",
    "made_cancel_ceremony",
    "made_enforce_ceremony_deadlines",
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
        "made_pause_ceremony" => {
            client
                .pause_ceremony(build_pause_ceremony_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_resume_ceremony" => {
            client
                .resume_ceremony(build_resume_ceremony_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_cancel_ceremony" => {
            client
                .cancel_ceremony(build_cancel_ceremony_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner()
                .instance
        }
        "made_enforce_ceremony_deadlines" => {
            client
                .enforce_ceremony_deadlines(
                    build_enforce_ceremony_deadlines_request(arguments).map_err(bad_request)?,
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
        .map(p2j::ceremony_instance_state_to_json)
        .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
}
