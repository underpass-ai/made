use made_core::error::DomainError;
use made_core::value_objects::{
    BudgetAccountId, BudgetBalance, BudgetLimits, BudgetMeasurement, BudgetQuantities,
    BudgetReservation, BudgetReservationEstimate, BudgetTokenCount, CostMicros, CurrencyCode,
    ExecutionDuration, ToolCallCount,
};

pub(super) fn budget_account_id_to_proto(value: Option<&BudgetAccountId>) -> String {
    value.map_or_else(String::new, |account| account.as_str().to_owned())
}
use made_proto::v1 as pb;

pub fn budget_limits_from_proto(value: pb::BudgetLimits) -> Result<BudgetLimits, DomainError> {
    let quantities = BudgetQuantities::new(
        ExecutionDuration::from_micros(value.duration_micros.unwrap_or_default()),
        BudgetTokenCount::new(value.tokens.unwrap_or_default()),
        CostMicros::new(value.cost_micros.unwrap_or_default()),
        ToolCallCount::new(value.tool_calls.unwrap_or_default()),
    );
    let currency = (!value.currency.trim().is_empty())
        .then(|| CurrencyCode::new(value.currency))
        .transpose()?;
    BudgetLimits::new(quantities, currency)
}

pub fn budget_reservation_estimate_from_proto(
    value: pb::BudgetReservationEstimate,
) -> Result<BudgetReservationEstimate, DomainError> {
    Ok(BudgetReservationEstimate::new(
        measurement(value.duration, ExecutionDuration::from_micros)?,
        measurement(value.tokens, BudgetTokenCount::new)?,
        measurement(value.cost, CostMicros::new)?,
        measurement(value.tool_calls, ToolCallCount::new)?,
    ))
}

fn measurement<T>(
    value: Option<pb::BudgetMeasurement>,
    construct: impl FnOnce(u64) -> T,
) -> Result<BudgetMeasurement<T>, DomainError> {
    let Some(value) = value else {
        return Ok(BudgetMeasurement::Unknown);
    };
    match value.quality.as_str() {
        "observed" => value
            .amount
            .map(|amount| BudgetMeasurement::Observed(construct(amount)))
            .ok_or(DomainError::InvalidCharacters {
                field: "budget_measurement.amount",
            }),
        "estimated" => value
            .amount
            .map(|amount| BudgetMeasurement::Estimated(construct(amount)))
            .ok_or(DomainError::InvalidCharacters {
                field: "budget_measurement.amount",
            }),
        "unknown" if value.amount.is_none() || value.amount == Some(0) => {
            Ok(BudgetMeasurement::Unknown)
        }
        _ => Err(DomainError::InvalidCharacters {
            field: "budget_measurement.quality",
        }),
    }
}

pub fn budget_balance_to_proto(value: &BudgetBalance) -> pb::BudgetBalance {
    pb::BudgetBalance {
        limits: Some(limits_to_proto(value.limits())),
        reserved: Some(quantities_to_proto(value.reserved())),
        observed: Some(quantities_to_proto(value.observed())),
        estimated: Some(quantities_to_proto(value.estimated())),
        unconfirmed: Some(quantities_to_proto(value.unconfirmed())),
        overrun: Some(quantities_to_proto(value.overrun())),
        available: Some(quantities_to_proto(value.available())),
    }
}

pub fn budget_reservation_to_proto(value: &BudgetReservation) -> pb::BudgetReservationRecord {
    pb::BudgetReservationRecord {
        reservation_id: value.id().as_str().to_owned(),
        operation_id: value.operation_id().as_str().to_owned(),
        quantities: Some(quantities_to_proto(value.quantities())),
        estimate: Some(estimate_to_proto(value.estimate())),
        reserved_at: super::ceremony_instance::moment(value.reserved_at()),
        reconciled: value.reconciliation().is_some(),
    }
}

fn limits_to_proto(value: &BudgetLimits) -> pb::BudgetLimits {
    pb::BudgetLimits {
        duration_micros: value.duration().map(ExecutionDuration::as_micros),
        tokens: value.tokens().map(BudgetTokenCount::value),
        cost_micros: value.cost().map(CostMicros::value),
        tool_calls: value.tool_calls().map(ToolCallCount::value),
        currency: value
            .currency()
            .map_or_else(String::new, |currency| currency.as_str().to_owned()),
    }
}

fn quantities_to_proto(value: BudgetQuantities) -> pb::BudgetQuantities {
    pb::BudgetQuantities {
        duration_micros: value.duration().as_micros(),
        tokens: value.tokens().value(),
        cost_micros: value.cost().value(),
        tool_calls: value.tool_calls().value(),
    }
}

fn estimate_to_proto(value: BudgetReservationEstimate) -> pb::BudgetReservationEstimate {
    pb::BudgetReservationEstimate {
        duration: Some(measurement_to_proto(
            value.duration(),
            ExecutionDuration::as_micros,
        )),
        tokens: Some(measurement_to_proto(
            value.tokens(),
            BudgetTokenCount::value,
        )),
        cost: Some(measurement_to_proto(value.cost(), CostMicros::value)),
        tool_calls: Some(measurement_to_proto(
            value.tool_calls(),
            ToolCallCount::value,
        )),
    }
}

fn measurement_to_proto<T: Copy>(
    value: BudgetMeasurement<T>,
    amount: impl FnOnce(T) -> u64,
) -> pb::BudgetMeasurement {
    match value {
        BudgetMeasurement::Observed(value) => pb::BudgetMeasurement {
            quality: "observed".to_owned(),
            amount: Some(amount(value)),
        },
        BudgetMeasurement::Estimated(value) => pb::BudgetMeasurement {
            quality: "estimated".to_owned(),
            amount: Some(amount(value)),
        },
        BudgetMeasurement::Unknown => pb::BudgetMeasurement {
            quality: "unknown".to_owned(),
            amount: None,
        },
    }
}
