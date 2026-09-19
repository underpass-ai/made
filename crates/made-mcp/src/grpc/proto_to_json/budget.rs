use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

pub(crate) fn budget_report_to_json(value: &pb::GetBudgetReportResponse) -> Value {
    json!({
        "account_id": value.account_id,
        "balance": value.balance.as_ref().map(balance_to_json),
    })
}

pub(crate) fn pending_budget_reservations_to_json(
    value: &pb::ListPendingBudgetReservationsResponse,
) -> Value {
    json!({
        "reservations": value.reservations.iter().map(reservation_to_json).collect::<Vec<_>>(),
        "next_cursor": empty_as_null(&value.next_cursor),
    })
}

pub(crate) fn admission_to_json(value: &pb::BudgetClaimAdmission) -> Value {
    json!({
        "account_id": value.account_id,
        "operation_id": value.operation_id,
        "reservation_id": value.reservation_id,
    })
}

fn balance_to_json(value: &pb::BudgetBalance) -> Value {
    json!({
        "limits": value.limits.as_ref().map(limits_to_json),
        "reserved": value.reserved.as_ref().map(quantities_to_json),
        "observed": value.observed.as_ref().map(quantities_to_json),
        "estimated": value.estimated.as_ref().map(quantities_to_json),
        "unconfirmed": value.unconfirmed.as_ref().map(quantities_to_json),
        "overrun": value.overrun.as_ref().map(quantities_to_json),
        "available": value.available.as_ref().map(quantities_to_json),
    })
}

fn limits_to_json(value: &pb::BudgetLimits) -> Value {
    json!({
        "duration_micros": value.duration_micros,
        "tokens": value.tokens,
        "cost_micros": value.cost_micros,
        "tool_calls": value.tool_calls,
        "currency": empty_as_null(&value.currency),
    })
}

fn quantities_to_json(value: &pb::BudgetQuantities) -> Value {
    json!({
        "duration_micros": value.duration_micros,
        "tokens": value.tokens,
        "cost_micros": value.cost_micros,
        "tool_calls": value.tool_calls,
    })
}

fn reservation_to_json(value: &pb::BudgetReservationRecord) -> Value {
    json!({
        "reservation_id": value.reservation_id,
        "operation_id": value.operation_id,
        "quantities": value.quantities.as_ref().map(quantities_to_json),
        "estimate": value.estimate.as_ref().map(estimate_to_json),
        "reserved_at": value.reserved_at,
        "reconciled": value.reconciled,
    })
}

fn estimate_to_json(value: &pb::BudgetReservationEstimate) -> Value {
    json!({
        "duration": value.duration.as_ref().map(measurement_to_json),
        "tokens": value.tokens.as_ref().map(measurement_to_json),
        "cost": value.cost.as_ref().map(measurement_to_json),
        "tool_calls": value.tool_calls.as_ref().map(measurement_to_json),
    })
}

fn measurement_to_json(value: &pb::BudgetMeasurement) -> Value {
    json!({"quality": value.quality, "amount": value.amount.unwrap_or_default()})
}

fn empty_as_null(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_owned())
    }
}
