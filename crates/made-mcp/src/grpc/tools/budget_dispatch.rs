use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::{ToolError, GET_BUDGET_REPORT_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL};

use super::super::{json_to_proto as j2p, proto_to_json as p2j};
use super::request_error::bad_request;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        GET_BUDGET_REPORT_TOOL | LIST_PENDING_BUDGET_RESERVATIONS_TOOL
    )
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let object = j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
    match name {
        "made_get_budget_report" => {
            let response = client
                .get_budget_report(pb::GetBudgetReportRequest {
                    ceremony_id: j2p::require_str(object, "ceremony_id")
                        .map_err(bad_request)?
                        .to_owned(),
                })
                .await?
                .into_inner();
            Ok(p2j::budget_report_to_json(&response))
        }
        "made_list_pending_budget_reservations" => {
            let response = client
                .list_pending_budget_reservations(pb::ListPendingBudgetReservationsRequest {
                    after_reservation_id: j2p::optional_str(object, "after_reservation_id")
                        .unwrap_or_default()
                        .to_owned(),
                    limit: j2p::optional_u32(object, "limit").map_err(bad_request)?,
                })
                .await?
                .into_inner();
            Ok(p2j::pending_budget_reservations_to_json(&response))
        }
        _ => Err(ToolError::invalid_request(format!(
            "unknown budget tool `{name}`"
        ))),
    }
}
