use made_core::value_objects::{
    BudgetLimits, BudgetMeasurement, BudgetReservationEstimate, BudgetTokenCount, CostMicros,
    CurrencyCode, ExecutionDuration, ToolCallCount,
};
use serde_json::{Map, Value};

pub(super) fn limits(object: &Map<String, Value>) -> Result<Option<BudgetLimits>, String> {
    object.get("budget_limits").map(parse_limits).transpose()
}

pub(super) fn reservation(
    object: &Map<String, Value>,
) -> Result<Option<BudgetReservationEstimate>, String> {
    object
        .get("budget_reservation")
        .map(parse_reservation)
        .transpose()
}

fn parse_limits(value: &Value) -> Result<BudgetLimits, String> {
    let object = parse_object(value, "budget_limits")?;
    let currency = object
        .get("currency")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "field `currency` must be a string".to_owned())
                .and_then(|value| CurrencyCode::new(value).map_err(|error| error.to_string()))
        })
        .transpose()?;
    BudgetLimits::from_optional(
        optional_u64(object, "duration_micros")?.map(ExecutionDuration::from_micros),
        optional_u64(object, "tokens")?.map(BudgetTokenCount::new),
        optional_u64(object, "cost_micros")?.map(CostMicros::new),
        optional_u64(object, "tool_calls")?.map(ToolCallCount::new),
        currency,
    )
    .map_err(|error| error.to_string())
}

fn parse_reservation(value: &Value) -> Result<BudgetReservationEstimate, String> {
    let object = parse_object(value, "budget_reservation")?;
    Ok(BudgetReservationEstimate::new(
        measurement(object, "duration", ExecutionDuration::from_micros)?,
        measurement(object, "tokens", BudgetTokenCount::new)?,
        measurement(object, "cost", CostMicros::new)?,
        measurement(object, "tool_calls", ToolCallCount::new)?,
    ))
}

fn measurement<T>(
    object: &Map<String, Value>,
    field: &str,
    construct: impl FnOnce(u64) -> T,
) -> Result<BudgetMeasurement<T>, String> {
    let value = object
        .get(field)
        .ok_or_else(|| format!("missing required field `{field}`"))?;
    let measurement = parse_object(value, field)?;
    let quality = measurement
        .get("quality")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("field `{field}.quality` must be a string"))?;
    let amount = optional_u64(measurement, "amount")?;
    match quality {
        "observed" => amount
            .map(|amount| BudgetMeasurement::Observed(construct(amount)))
            .ok_or_else(|| format!("field `{field}.amount` is required for observed quality")),
        "estimated" => amount
            .map(|amount| BudgetMeasurement::Estimated(construct(amount)))
            .ok_or_else(|| format!("field `{field}.amount` is required for estimated quality")),
        "unknown" if amount.is_none() || amount == Some(0) => Ok(BudgetMeasurement::Unknown),
        _ => Err(format!("field `{field}.quality` is invalid")),
    }
}

fn parse_object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("field `{field}` must be an object"))
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, String> {
    object
        .get(field)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("field `{field}` must be an unsigned integer"))
        })
        .transpose()
}
