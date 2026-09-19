use made_client::v1::{
    BudgetBalance, BudgetLimits, BudgetMeasurement, BudgetQuantities, BudgetReservationEstimate,
    GetBudgetReportResponse,
};
use serde_json::{json, Value};

pub(super) fn budget_report_value(report: &GetBudgetReportResponse) -> Value {
    let blocked_dimensions = report
        .balance
        .as_ref()
        .map(blocked_dimensions)
        .unwrap_or_default();
    json!({
        "account_id": report.account_id,
        "balance": report.balance.as_ref().map(balance_value),
        "admission": {
            "blocked": !blocked_dimensions.is_empty(),
            "blocked_dimensions": blocked_dimensions,
            "explanation": if blocked_dimensions.is_empty() {
                "no exhausted or overrun bounded dimension"
            } else {
                "a bounded dimension is exhausted or overrun; inspect pending reservations before retrying"
            },
        },
    })
}

fn balance_value(balance: &BudgetBalance) -> Value {
    json!({
        "limits": balance.limits.as_ref().map(limits_value),
        "reserved": balance.reserved.as_ref().map(quantities_value),
        "observed": balance.observed.as_ref().map(quantities_value),
        "estimated": balance.estimated.as_ref().map(quantities_value),
        "unconfirmed": balance.unconfirmed.as_ref().map(quantities_value),
        "overrun": balance.overrun.as_ref().map(quantities_value),
        "available": balance.available.as_ref().map(quantities_value),
    })
}

fn limits_value(limits: &BudgetLimits) -> Value {
    json!({
        "duration_micros": limits.duration_micros,
        "tokens": limits.tokens,
        "cost_micros": limits.cost_micros,
        "tool_calls": limits.tool_calls,
        "currency": limits.currency,
    })
}

pub(super) fn quantities_value(quantities: &BudgetQuantities) -> Value {
    json!({
        "duration_micros": quantities.duration_micros,
        "tokens": quantities.tokens,
        "cost_micros": quantities.cost_micros,
        "tool_calls": quantities.tool_calls,
    })
}

pub(super) fn estimate_value(estimate: &BudgetReservationEstimate) -> Value {
    json!({
        "duration": estimate.duration.as_ref().map(measurement_value),
        "tokens": estimate.tokens.as_ref().map(measurement_value),
        "cost": estimate.cost.as_ref().map(measurement_value),
        "tool_calls": estimate.tool_calls.as_ref().map(measurement_value),
    })
}

fn measurement_value(measurement: &BudgetMeasurement) -> Value {
    json!({
        "quality": measurement.quality,
        "amount": measurement.amount,
    })
}

fn blocked_dimensions(balance: &BudgetBalance) -> Vec<&'static str> {
    let Some(limits) = &balance.limits else {
        return Vec::new();
    };
    let available = balance.available.as_ref();
    let overrun = balance.overrun.as_ref();
    let mut blocked = Vec::new();
    if dimension_blocked(
        limits.duration_micros,
        available.map(|value| value.duration_micros),
        overrun.map(|value| value.duration_micros),
    ) {
        blocked.push("duration_micros");
    }
    if dimension_blocked(
        limits.tokens,
        available.map(|value| value.tokens),
        overrun.map(|value| value.tokens),
    ) {
        blocked.push("tokens");
    }
    if dimension_blocked(
        limits.cost_micros,
        available.map(|value| value.cost_micros),
        overrun.map(|value| value.cost_micros),
    ) {
        blocked.push("cost_micros");
    }
    if dimension_blocked(
        limits.tool_calls,
        available.map(|value| value.tool_calls),
        overrun.map(|value| value.tool_calls),
    ) {
        blocked.push("tool_calls");
    }
    blocked
}

fn dimension_blocked(limit: Option<u64>, available: Option<u64>, overrun: Option<u64>) -> bool {
    limit.is_some() && (available == Some(0) || overrun.is_some_and(|value| value > 0))
}
