use made_core::value_objects::{BudgetPageLimit, BudgetReservationId, CeremonyId};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use crate::protocol::{ToolError, GET_BUDGET_REPORT_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        GET_BUDGET_REPORT_TOOL | LIST_PENDING_BUDGET_RESERVATIONS_TOOL
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))?;
    match name {
        GET_BUDGET_REPORT_TOOL => {
            let ceremony_id = CeremonyId::new(required_string(object, "ceremony_id")?)?;
            let instance = made.instance(&ceremony_id).await?;
            let account = instance
                .budget_account_id()
                .ok_or_else(|| ToolError::refused("ceremony has no durable budget account"))?;
            let balance = made.budget_report(account).await?;
            let balance = serde_json::to_value(balance)
                .map_err(|error| ToolError::unavailable(error.to_string()))?;
            Ok(json!({"account_id": account.as_str(), "balance": balance}))
        }
        LIST_PENDING_BUDGET_RESERVATIONS_TOOL => {
            let after = object
                .get("after_reservation_id")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| {
                            ToolError::invalid_request(
                                "field `after_reservation_id` must be a string",
                            )
                        })
                        .and_then(|value| BudgetReservationId::new(value).map_err(Into::into))
                })
                .transpose()?;
            let limit = object.get("limit").map_or(Ok(100_u64), |value| {
                value.as_u64().ok_or_else(|| {
                    ToolError::invalid_request("field `limit` must be an unsigned integer")
                })
            })?;
            let page = made
                .pending_budget_reservations(after.as_ref(), BudgetPageLimit::new(limit as usize)?)
                .await?;
            let next_cursor = page.reservations().last().map(|value| value.id().as_str());
            let reservations = page
                .reservations()
                .iter()
                .map(|value| {
                    json!({
                        "reservation_id": value.id().as_str(),
                        "operation_id": value.operation_id().as_str(),
                        "quantities": value.quantities(),
                        "estimate": value.estimate(),
                        "reserved_at": value.reserved_at(),
                        "reconciled": value.reconciliation().is_some(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({"reservations": reservations, "next_cursor": next_cursor}))
        }
        _ => Err(ToolError::invalid_request(format!(
            "unknown budget tool `{name}`"
        ))),
    }
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, ToolError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ToolError::invalid_request(format!("field `{field}` is required")))
}
