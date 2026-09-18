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
