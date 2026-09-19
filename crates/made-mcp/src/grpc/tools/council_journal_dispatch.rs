use crate::grpc::proto_to_json::authorization_evidence_to_json;
use crate::protocol::ToolError;
use made_mcp_proto::v1::{self as pb, made_service_client::MadeServiceClient};
use serde_json::{json, Value};
use tonic::transport::Channel;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_read_council_events"
            | "made_get_council_event_cursor"
            | "made_lease_council_events"
            | "made_acknowledge_council_events"
            | "made_release_council_events"
    )
}
fn unsigned(args: &Value, field: &str) -> Result<u64, ToolError> {
    args.get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::invalid_request(format!("{field} must be an unsigned integer")))
}
fn string(args: &Value, field: &str) -> Result<String, ToolError> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ToolError::invalid_request(format!("{field} must be a string")))
}
fn lease(args: &Value) -> Result<pb::CouncilJournalLease, ToolError> {
    let lease = args
        .get("lease")
        .ok_or_else(|| ToolError::invalid_request("lease is required"))?;
    Ok(pb::CouncilJournalLease {
        consumer: string(lease, "consumer")?,
        lease_id: string(lease, "id")?,
        expires_at: string(lease, "expires_at")?,
        acknowledged_through: match lease.get("acknowledged_through") {
            None | Some(Value::Null) => None,
            _ => Some(unsigned(lease, "acknowledged_through")?),
        },
    })
}
pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_read_council_events" => {
            let response = client
                .read_council_events(pb::ReadCouncilEventsRequest {
                    after: args
                        .get("after")
                        .map(|_| unsigned(args, "after"))
                        .transpose()?,
                    limit: u32::try_from(
                        args.get("limit")
                            .map(|_| unsigned(args, "limit"))
                            .transpose()?
                            .unwrap_or(200),
                    )
                    .map_err(|_| ToolError::invalid_request("limit is too large"))?,
                })
                .await?
                .into_inner();
            let records = response
                .records
                .into_iter()
                .map(|record| {
                    let event: Value =
                        serde_json::from_str(&record.event_json).map_err(|error| {
                            ToolError::refused(format!("invalid council journal event: {error}"))
                        })?;
                    let mut value = json!({"position":record.position,"event":event});
                    if let (Some(object), Some(authorization)) =
                        (value.as_object_mut(), record.authorization.as_ref())
                    {
                        object.insert(
                            "authorization".to_owned(),
                            authorization_evidence_to_json(authorization),
                        );
                    }
                    Ok(value)
                })
                .collect::<Result<Vec<_>, ToolError>>()?;
            Ok(json!({"records":records,"next_after":response.next_after}))
        }
        "made_get_council_event_cursor" => {
            let response = client
                .get_council_event_cursor(pb::GetCouncilEventCursorRequest {
                    consumer: string(args, "consumer")?,
                })
                .await?
                .into_inner();
            Ok(json!({"acknowledged_through":response.acknowledged_through}))
        }
        "made_lease_council_events" => {
            let response = client
                .lease_council_events(pb::LeaseCouncilEventsRequest {
                    consumer: string(args, "consumer")?,
                    duration_ms: unsigned(args, "duration_ms")?,
                })
                .await?
                .into_inner();
            Ok(
                json!({"lease":response.lease.map(|lease| json!({"consumer":lease.consumer,"id":lease.lease_id,"acknowledged_through":lease.acknowledged_through,"expires_at":lease.expires_at}))}),
            )
        }
        "made_acknowledge_council_events" => {
            client
                .acknowledge_council_events(pb::AcknowledgeCouncilEventsRequest {
                    lease: Some(lease(args)?),
                    through: unsigned(args, "through")?,
                })
                .await?;
            Ok(json!({}))
        }
        "made_release_council_events" => {
            client
                .release_council_events(pb::ReleaseCouncilEventsRequest {
                    lease: Some(lease(args)?),
                })
                .await?;
            Ok(json!({}))
        }
        _ => Err(ToolError::invalid_request("unknown council journal tool")),
    }
}
