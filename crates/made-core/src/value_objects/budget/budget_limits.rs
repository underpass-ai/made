use super::{
    BudgetQuantities, BudgetTokenCount, CostMicros, CurrencyCode, ExecutionDuration, ToolCallCount,
};
use crate::DomainError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetLimits {
    duration: Option<ExecutionDuration>,
    tokens: Option<BudgetTokenCount>,
    cost: Option<CostMicros>,
    tool_calls: Option<ToolCallCount>,
    currency: Option<CurrencyCode>,
}
impl BudgetLimits {
    /// Preserve presence at public boundaries: a supplied ceiling must be positive.
    /// Only absence means that the dimension has no ceiling.
    pub fn from_optional(
        duration: Option<ExecutionDuration>,
        tokens: Option<BudgetTokenCount>,
        cost: Option<CostMicros>,
        tool_calls: Option<ToolCallCount>,
        currency: Option<CurrencyCode>,
    ) -> Result<Self, DomainError> {
        for (field, amount) in [
            (
                "budget_limits.duration_micros",
                duration.map(ExecutionDuration::as_micros),
            ),
            ("budget_limits.tokens", tokens.map(BudgetTokenCount::value)),
            ("budget_limits.cost_micros", cost.map(CostMicros::value)),
            (
                "budget_limits.tool_calls",
                tool_calls.map(ToolCallCount::value),
            ),
        ] {
            if amount == Some(0) {
                return Err(DomainError::MustBeNonZero { field });
            }
        }
        Self::new(
            BudgetQuantities::new(
                duration.unwrap_or_else(|| ExecutionDuration::from_micros(0)),
                tokens.unwrap_or_else(|| BudgetTokenCount::new(0)),
                cost.unwrap_or_else(|| CostMicros::new(0)),
                tool_calls.unwrap_or_else(|| ToolCallCount::new(0)),
            ),
            currency,
        )
    }

    pub fn new(
        maximum: BudgetQuantities,
        currency: Option<CurrencyCode>,
    ) -> Result<Self, DomainError> {
        if maximum.is_zero() {
            return Err(DomainError::EmptyCollection {
                field: "budget_limits",
            });
        }
        if (maximum.cost().value() > 0) != currency.is_some() {
            return Err(DomainError::InvariantViolated {
                reason: "a cost budget and its currency must be declared together",
            });
        }
        Ok(Self {
            duration: (maximum.duration().as_micros() > 0).then_some(maximum.duration()),
            tokens: (maximum.tokens().value() > 0).then_some(maximum.tokens()),
            cost: (maximum.cost().value() > 0).then_some(maximum.cost()),
            tool_calls: (maximum.tool_calls().value() > 0).then_some(maximum.tool_calls()),
            currency,
        })
    }
    #[must_use]
    pub const fn duration(&self) -> Option<ExecutionDuration> {
        self.duration
    }
    #[must_use]
    pub const fn tokens(&self) -> Option<BudgetTokenCount> {
        self.tokens
    }
    #[must_use]
    pub const fn cost(&self) -> Option<CostMicros> {
        self.cost
    }
    #[must_use]
    pub const fn tool_calls(&self) -> Option<ToolCallCount> {
        self.tool_calls
    }
    #[must_use]
    pub const fn currency(&self) -> Option<&CurrencyCode> {
        self.currency.as_ref()
    }
}
