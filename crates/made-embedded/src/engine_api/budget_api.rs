use made_api::{
    ApiError, BudgetBalance as BudgetBalanceView, BudgetLimits as BudgetLimitsView,
    BudgetQuantities as BudgetQuantitiesView, BudgetReport,
    BudgetReservation as BudgetReservationView, CeremonySummary, StartCeremonyRequest,
};
use made_app::budgets::StartBudgetedCeremonyInput;
use made_core::error::DomainError;
use made_core::value_objects::{
    BudgetLimits, BudgetPageLimit, BudgetQuantities, BudgetReservationId, BudgetTokenCount,
    CeremonyId, CostMicros, CurrencyCode, ExecutionDuration, ToolCallCount,
};

use super::{millis, start_input, summarize, unavailable};
use crate::EmbeddedMade;

pub(super) async fn start(
    made: &EmbeddedMade,
    request: StartCeremonyRequest,
    limits: BudgetLimitsView,
) -> Result<CeremonySummary, ApiError> {
    let input = start_input(request).map_err(|error| refused(&error))?;
    let limits = budget_limits(limits).map_err(|error| refused(&error))?;
    made.start_budgeted_published(StartBudgetedCeremonyInput::new(input, limits))
        .await
        .map(|instance| summarize(&instance))
        .map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })
}

pub(super) async fn report(
    made: &EmbeddedMade,
    ceremony_id: &str,
) -> Result<BudgetReport, ApiError> {
    let ceremony_id = CeremonyId::new(ceremony_id).map_err(|error| refused(&error))?;
    let instance = made
        .instance(&ceremony_id)
        .await
        .map_err(|error| match error {
            DomainError::NotFound { .. } => ApiError::CeremonyNotFound {
                ceremony_id: ceremony_id.as_str().to_owned(),
            },
            other => unavailable(&other),
        })?;
    let account = instance
        .budget_account_id()
        .ok_or_else(|| ApiError::Refused {
            reason: "ceremony has no durable budget account".to_owned(),
        })?;
    let balance = made
        .budget_report(account)
        .await
        .map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
    Ok(BudgetReport {
        account_id: account.as_str().to_owned(),
        balance: budget_balance(&balance),
    })
}

pub(super) async fn pending(
    made: &EmbeddedMade,
    after_reservation_id: Option<&str>,
    limit: usize,
) -> Result<Vec<BudgetReservationView>, ApiError> {
    let after = after_reservation_id
        .map(BudgetReservationId::new)
        .transpose()
        .map_err(|error| refused(&error))?;
    let limit = BudgetPageLimit::new(limit).map_err(|error| refused(&error))?;
    made.pending_budget_reservations(after.as_ref(), limit)
        .await
        .map(|page| page.reservations().iter().map(budget_reservation).collect())
        .map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })
}

fn refused(error: &DomainError) -> ApiError {
    ApiError::Refused {
        reason: error.to_string(),
    }
}

fn budget_limits(value: BudgetLimitsView) -> Result<BudgetLimits, DomainError> {
    let maximum = BudgetQuantities::new(
        ExecutionDuration::from_micros(value.duration_micros.unwrap_or_default()),
        BudgetTokenCount::new(value.tokens.unwrap_or_default()),
        CostMicros::new(value.cost_micros.unwrap_or_default()),
        ToolCallCount::new(value.tool_calls.unwrap_or_default()),
    );
    let currency = value.currency.map(CurrencyCode::new).transpose()?;
    BudgetLimits::new(maximum, currency)
}

fn budget_balance(value: &made_core::value_objects::BudgetBalance) -> BudgetBalanceView {
    BudgetBalanceView {
        limits: BudgetLimitsView {
            duration_micros: value.limits().duration().map(ExecutionDuration::as_micros),
            tokens: value.limits().tokens().map(BudgetTokenCount::value),
            cost_micros: value.limits().cost().map(CostMicros::value),
            tool_calls: value.limits().tool_calls().map(ToolCallCount::value),
            currency: value
                .limits()
                .currency()
                .map(|value| value.as_str().to_owned()),
        },
        reserved: budget_quantities(value.reserved()),
        observed: budget_quantities(value.observed()),
        estimated: budget_quantities(value.estimated()),
        unconfirmed: budget_quantities(value.unconfirmed()),
        overrun: budget_quantities(value.overrun()),
        available: budget_quantities(value.available()),
    }
}

fn budget_quantities(value: BudgetQuantities) -> BudgetQuantitiesView {
    BudgetQuantitiesView {
        duration_micros: value.duration().as_micros(),
        tokens: value.tokens().value(),
        cost_micros: value.cost().value(),
        tool_calls: value.tool_calls().value(),
    }
}

fn budget_reservation(
    value: &made_core::value_objects::BudgetReservation,
) -> BudgetReservationView {
    BudgetReservationView {
        reservation_id: value.id().as_str().to_owned(),
        operation_id: value.operation_id().as_str().to_owned(),
        quantities: budget_quantities(value.quantities()),
        reserved_at_millis: millis(value.reserved_at()),
        reconciled: value.reconciliation().is_some(),
    }
}
