use crate::protocol::ToolError;
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalPageLimit, CouncilJournalPosition,
    DurationMs,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

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
fn consumer(args: &Value) -> Result<CouncilJournalConsumer, ToolError> {
    Ok(CouncilJournalConsumer::new(
        args.get("consumer")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::invalid_request("consumer is required"))?,
    )?)
}
fn lease(args: &Value) -> Result<CouncilJournalLease, ToolError> {
    serde_json::from_value(
        args.get("lease")
            .cloned()
            .ok_or_else(|| ToolError::invalid_request("lease is required"))?,
    )
    .map_err(|error| ToolError::invalid_request(error.to_string()))
}
pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_read_council_events" => {
            let after = args
                .get("after")
                .map(|_| unsigned(args, "after"))
                .transpose()?
                .map(CouncilJournalPosition::new)
                .transpose()?;
            let limit = args
                .get("limit")
                .map(|_| unsigned(args, "limit"))
                .transpose()?
                .unwrap_or(200);
            let limit = usize::try_from(limit)
                .map_err(|_| ToolError::invalid_request("limit is too large"))?;
            let records = made
                .read_council_events(after, CouncilJournalPageLimit::new(limit)?)
                .await?;
            let next_after = records
                .last()
                .map(made_core::entities::CouncilJournalRecord::position)
                .or(after);
            Ok(json!({"records":records,"next_after":next_after}))
        }
        "made_get_council_event_cursor" => Ok(
            json!({"acknowledged_through":made.get_council_event_cursor(&consumer(args)?).await?}),
        ),
        "made_lease_council_events" => Ok(
            json!({"lease": made.lease_council_events(&consumer(args)?, DurationMs::from_millis(unsigned(args,"duration_ms")?)).await?}),
        ),
        "made_acknowledge_council_events" => {
            made.acknowledge_council_events(
                &lease(args)?,
                CouncilJournalPosition::new(unsigned(args, "through")?)?,
            )
            .await?;
            Ok(json!({}))
        }
        "made_release_council_events" => {
            made.release_council_events(&lease(args)?).await?;
            Ok(json!({}))
        }
        _ => Err(ToolError::invalid_request("unknown council journal tool")),
    }
}
