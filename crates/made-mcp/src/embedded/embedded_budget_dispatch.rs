use made_core::ports::AuthorizationScopeResolverPort;
use made_core::value_objects::{
    AuthorizationScope, BudgetBalance, BudgetLimits, BudgetMeasurement, BudgetPageLimit,
    BudgetQuantities, BudgetReservation, BudgetReservationEstimate, BudgetReservationId,
    BudgetTokenCount, CeremonyId, CostMicros, ExecutionDuration, ToolCallCount,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;

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
            let scope = made.budget_scope(&ceremony_id).await?;
            let AuthorizationScope::Budget { account_id } = scope else {
                return Err(ToolError::refused("ceremony has no durable budget account"));
            };
            let balance = made.budget_report(&account_id).await?;
            Ok(json!({
                "account_id": account_id.as_str(),
                "balance": balance_to_json(&balance),
            }))
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
                .map(reservation_to_json)
                .collect::<Vec<_>>();
            Ok(json!({"reservations": reservations, "next_cursor": next_cursor}))
        }
        _ => Err(ToolError::invalid_request(format!(
            "unknown budget tool `{name}`"
        ))),
    }
}

fn balance_to_json(value: &BudgetBalance) -> Value {
    json!({
        "limits": limits_to_json(value.limits()),
        "reserved": quantities_to_json(value.reserved()),
        "observed": quantities_to_json(value.observed()),
        "estimated": quantities_to_json(value.estimated()),
        "unconfirmed": quantities_to_json(value.unconfirmed()),
        "overrun": quantities_to_json(value.overrun()),
        "available": quantities_to_json(value.available()),
    })
}

fn limits_to_json(value: &BudgetLimits) -> Value {
    json!({
        "duration_micros": value.duration().map(ExecutionDuration::as_micros),
        "tokens": value.tokens().map(BudgetTokenCount::value),
        "cost_micros": value.cost().map(CostMicros::value),
        "tool_calls": value.tool_calls().map(ToolCallCount::value),
        "currency": value.currency().map(made_core::value_objects::CurrencyCode::as_str),
    })
}

fn quantities_to_json(value: BudgetQuantities) -> Value {
    json!({
        "duration_micros": value.duration().as_micros(),
        "tokens": value.tokens().value(),
        "cost_micros": value.cost().value(),
        "tool_calls": value.tool_calls().value(),
    })
}

fn reservation_to_json(value: &BudgetReservation) -> Value {
    json!({
        "reservation_id": value.id().as_str(),
        "operation_id": value.operation_id().as_str(),
        "quantities": quantities_to_json(value.quantities()),
        "estimate": estimate_to_json(value.estimate()),
        "reserved_at": value.reserved_at().format(&Rfc3339).unwrap_or_default(),
        "reconciled": value.reconciliation().is_some(),
    })
}

fn estimate_to_json(value: BudgetReservationEstimate) -> Value {
    json!({
        "duration": duration_measurement_to_json(value.duration()),
        "tokens": token_measurement_to_json(value.tokens()),
        "cost": cost_measurement_to_json(value.cost()),
        "tool_calls": tool_call_measurement_to_json(value.tool_calls()),
    })
}

fn duration_measurement_to_json(value: BudgetMeasurement<ExecutionDuration>) -> Value {
    match value {
        BudgetMeasurement::Observed(amount) => measurement_to_json("observed", amount.as_micros()),
        BudgetMeasurement::Estimated(amount) => {
            measurement_to_json("estimated", amount.as_micros())
        }
        BudgetMeasurement::Unknown => measurement_to_json("unknown", 0),
    }
}

fn token_measurement_to_json(value: BudgetMeasurement<BudgetTokenCount>) -> Value {
    match value {
        BudgetMeasurement::Observed(amount) => measurement_to_json("observed", amount.value()),
        BudgetMeasurement::Estimated(amount) => measurement_to_json("estimated", amount.value()),
        BudgetMeasurement::Unknown => measurement_to_json("unknown", 0),
    }
}

fn cost_measurement_to_json(value: BudgetMeasurement<CostMicros>) -> Value {
    match value {
        BudgetMeasurement::Observed(amount) => measurement_to_json("observed", amount.value()),
        BudgetMeasurement::Estimated(amount) => measurement_to_json("estimated", amount.value()),
        BudgetMeasurement::Unknown => measurement_to_json("unknown", 0),
    }
}

fn tool_call_measurement_to_json(value: BudgetMeasurement<ToolCallCount>) -> Value {
    match value {
        BudgetMeasurement::Observed(amount) => measurement_to_json("observed", amount.value()),
        BudgetMeasurement::Estimated(amount) => measurement_to_json("estimated", amount.value()),
        BudgetMeasurement::Unknown => measurement_to_json("unknown", 0),
    }
}

fn measurement_to_json(quality: &str, amount: u64) -> Value {
    json!({"quality": quality, "amount": amount})
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
