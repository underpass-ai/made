use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, budget_dispatch, j2p, p2j};

pub(super) fn handles(name: &str) -> bool {
    budget_dispatch::handles(name)
        || matches!(
            name,
            "made_get_ceremony_instance"
                | "made_list_ceremony_instances"
                | "made_search_ceremony_instances"
        )
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    if budget_dispatch::handles(name) {
        return budget_dispatch::dispatch(client, name, arguments).await;
    }
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
        "made_search_ceremony_instances" => {
            let obj =
                j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
            let lifecycle = match j2p::optional_str(obj, "lifecycle") {
                None => pb::CeremonyLifecycleFilter::Unspecified,
                Some("running") => pb::CeremonyLifecycleFilter::Running,
                Some("paused") => pb::CeremonyLifecycleFilter::Paused,
                Some("ended") => pb::CeremonyLifecycleFilter::Ended,
                Some(other) => {
                    return Err(bad_request(format!(
                        "`lifecycle` must be running, paused or ended, got `{other}`"
                    )))
                }
            };
            let response = client
                .search_ceremony_instances(pb::SearchCeremonyInstancesRequest {
                    cursor: j2p::optional_str(obj, "cursor")
                        .unwrap_or_default()
                        .to_owned(),
                    limit: j2p::optional_u32(obj, "limit").map_err(bad_request)?,
                    id_prefix: j2p::optional_str(obj, "id_prefix")
                        .unwrap_or_default()
                        .to_owned(),
                    lifecycle: lifecycle as i32,
                })
                .await?
                .into_inner();
            let entries = response
                .instances
                .into_iter()
                .map(p2j::ceremony_instance_listing_entry)
                .collect();
            Ok(crate::renderers::CeremonyInstanceSearchPage::new(
                entries,
                (!response.next_cursor.is_empty()).then_some(response.next_cursor),
            )
            .to_json())
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown ceremony read tool `{other}`"
        ))),
    }
}
