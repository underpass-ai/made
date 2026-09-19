use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

pub(crate) fn budget_report_to_json(value: pb::GetBudgetReportResponse) -> Value {
    json!({
        "account_id": value.account_id,
        "balance": value.balance.map(balance_to_json),
    })
}

pub(crate) fn pending_budget_reservations_to_json(
    value: pb::ListPendingBudgetReservationsResponse,
) -> Value {
    json!({
        "reservations": value.reservations.into_iter().map(reservation_to_json).collect::<Vec<_>>(),
        "next_cursor": empty_as_null(value.next_cursor),
    })
}

pub(crate) fn admission_to_json(value: pb::BudgetClaimAdmission) -> Value {
    json!({
        "account_id": value.account_id,
        "operation_id": value.operation_id,
        "reservation_id": value.reservation_id,
    })
}

fn balance_to_json(value: pb::BudgetBalance) -> Value {
    json!({
        "limits": value.limits.map(limits_to_json),
        "reserved": value.reserved.map(quantities_to_json),
        "observed": value.observed.map(quantities_to_json),
        "estimated": value.estimated.map(quantities_to_json),
        "unconfirmed": value.unconfirmed.map(quantities_to_json),
        "overrun": value.overrun.map(quantities_to_json),
        "available": value.available.map(quantities_to_json),
    })
}

fn limits_to_json(value: pb::BudgetLimits) -> Value {
    json!({
        "duration_micros": value.duration_micros,
        "tokens": value.tokens,
        "cost_micros": value.cost_micros,
        "tool_calls": value.tool_calls,
        "currency": empty_as_null(value.currency),
    })
}

fn quantities_to_json(value: pb::BudgetQuantities) -> Value {
    json!({
        "duration_micros": value.duration_micros,
        "tokens": value.tokens,
        "cost_micros": value.cost_micros,
        "tool_calls": value.tool_calls,
    })
}

fn reservation_to_json(value: pb::BudgetReservationRecord) -> Value {
    json!({
        "reservation_id": value.reservation_id,
        "operation_id": value.operation_id,
        "quantities": value.quantities.map(quantities_to_json),
        "estimate": value.estimate.map(estimate_to_json),
        "reserved_at": value.reserved_at,
        "reconciled": value.reconciled,
    })
}

fn estimate_to_json(value: pb::BudgetReservationEstimate) -> Value {
    json!({
        "duration": value.duration.map(measurement_to_json),
        "tokens": value.tokens.map(measurement_to_json),
        "cost": value.cost.map(measurement_to_json),
        "tool_calls": value.tool_calls.map(measurement_to_json),
    })
}

fn measurement_to_json(value: pb::BudgetMeasurement) -> Value {
    json!({"quality": value.quality, "amount": value.amount})
}

fn empty_as_null(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}
