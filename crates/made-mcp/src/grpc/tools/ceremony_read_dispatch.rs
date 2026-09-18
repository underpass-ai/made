use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, j2p, p2j};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_get_ceremony_instance" | "made_list_ceremony_instances"
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
        "made_get_ceremony_instance" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let response = client
                .get_ceremony_instance(pb::GetCeremonyInstanceRequest {
                    ceremony_id: j2p::require_str(obj, "ceremony_id")
                        .map_err(bad_request)?
                        .to_owned(),
                })
                .await?
                .into_inner();
            response
                .instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }
        "made_list_ceremony_instances" => {
            let response = client
                .list_ceremony_instances(pb::ListCeremonyInstancesRequest {})
                .await?
                .into_inner();
            let entries = response
                .instances
                .into_iter()
                .map(p2j::ceremony_instance_listing_entry)
                .collect();
            Ok(crate::renderers::CeremonyInstanceListing::new(entries).to_json())
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown ceremony read tool `{other}`"
        ))),
    }
}
