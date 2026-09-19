use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};

use crate::grpc::json_to_proto as j2p;

pub(super) fn limits_from_json(value: &Value) -> Result<pb::BudgetLimits, String> {
    let object = j2p::require_object(value, "budget_limits")?;
    Ok(pb::BudgetLimits {
        duration_micros: present_u64(object, "duration_micros")?,
        tokens: present_u64(object, "tokens")?,
        cost_micros: present_u64(object, "cost_micros")?,
        tool_calls: present_u64(object, "tool_calls")?,
        currency: j2p::optional_str(object, "currency")
            .unwrap_or_default()
            .to_owned(),
    })
}

pub(super) fn reservation_from_json(
    value: &Value,
) -> Result<pb::BudgetReservationEstimate, String> {
    let object = j2p::require_object(value, "budget_reservation")?;
    Ok(pb::BudgetReservationEstimate {
        duration: Some(measurement(object, "duration")?),
        tokens: Some(measurement(object, "tokens")?),
        cost: Some(measurement(object, "cost")?),
        tool_calls: Some(measurement(object, "tool_calls")?),
    })
}

fn measurement(object: &Map<String, Value>, field: &str) -> Result<pb::BudgetMeasurement, String> {
    let value = object
        .get(field)
        .ok_or_else(|| format!("missing required field `{field}`"))?;
    let measurement = j2p::require_object(value, field)?;
    let quality = j2p::require_str(measurement, "quality")?;
    let amount = present_u64(measurement, "amount")?;
    match quality {
        "observed" | "estimated" if amount.is_none() => Err(format!(
            "field `{field}.amount` is required for {quality} quality"
        )),
        "unknown" if amount.is_some_and(|amount| amount != 0) => Err(format!(
            "field `{field}.amount` must be zero for unknown quality"
        )),
        "observed" | "estimated" | "unknown" => Ok(pb::BudgetMeasurement {
            quality: quality.to_owned(),
            amount,
        }),
        _ => Err(format!("field `{field}.quality` is invalid")),
    }
}

fn present_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, String> {
    object
        .get(field)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("field `{field}` must be an unsigned integer"))
        })
        .transpose()
}
