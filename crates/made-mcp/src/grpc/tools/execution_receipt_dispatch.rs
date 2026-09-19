use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::Value;
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::{bad_request, j2p, p2j};

const TOOLS: [&str; 4] = [
    "made_get_execution_receipt",
    "made_inspect_execution_recovery",
    "made_complete_execution_receipt",
    "made_adopt_execution_receipt",
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
    let object = j2p::require_object(arguments, "tools/call.arguments").map_err(bad_request)?;
    match name {
        "made_get_execution_receipt" => {
            let response = client
                .get_execution_receipt(pb::GetExecutionReceiptRequest {
                    operation_id: required(object, "operation_id")?,
                })
                .await?
                .into_inner();
            response
                .receipt
                .as_ref()
                .map(p2j::execution_receipt_to_json)
                .ok_or_else(|| ToolError::refused("made returned no execution receipt"))
        }
        "made_inspect_execution_recovery" => {
            let response = client
                .inspect_execution_recovery(pb::InspectExecutionRecoveryRequest {
                    after: j2p::optional_str(object, "after").map(ToOwned::to_owned),
                    limit: j2p::optional_u32(object, "limit").map_err(bad_request)?,
                })
                .await?
                .into_inner();
            Ok(p2j::execution_recovery_page_to_json(&response))
        }
        "made_complete_execution_receipt" => {
            let request = pb::CompleteExecutionReceiptRequest {
                ceremony_id: required(object, "ceremony_id")?,
                step_id: required(object, "step_id")?,
                operation_id: required(object, "operation_id")?,
                claim_fence: required(object, "claim_fence")?,
                actor_kind: required(object, "actor_kind")?,
            };
            let instance = client
                .complete_execution_receipt(request)
                .await?
                .into_inner()
                .instance;
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }
        "made_adopt_execution_receipt" => {
            let request = pb::AdoptExecutionReceiptRequest {
                ceremony_id: required(object, "ceremony_id")?,
                step_id: required(object, "step_id")?,
                operation_id: required(object, "operation_id")?,
                claim_fence: required(object, "claim_fence")?,
                actor_kind: required(object, "actor_kind")?,
            };
            let instance = client
                .adopt_execution_receipt(request)
                .await?
                .into_inner()
                .instance;
            instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown execution receipt tool `{other}`"
        ))),
    }
}

fn required(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, ToolError> {
    j2p::require_str(object, field)
        .map(ToOwned::to_owned)
        .map_err(bad_request)
}
