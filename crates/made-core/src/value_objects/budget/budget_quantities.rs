use super::{BudgetTokenCount, CostMicros, ExecutionDuration, ToolCallCount};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetQuantities {
    duration: ExecutionDuration,
    tokens: BudgetTokenCount,
    cost: CostMicros,
    tool_calls: ToolCallCount,
}
impl BudgetQuantities {
    #[must_use]
    pub const fn new(
        duration: ExecutionDuration,
        tokens: BudgetTokenCount,
        cost: CostMicros,
        tool_calls: ToolCallCount,
    ) -> Self {
        Self {
            duration,
            tokens,
            cost,
            tool_calls,
        }
    }
    #[must_use]
    pub const fn duration(self) -> ExecutionDuration {
        self.duration
    }
    #[must_use]
    pub const fn tokens(self) -> BudgetTokenCount {
        self.tokens
    }
    #[must_use]
    pub const fn cost(self) -> CostMicros {
        self.cost
    }
    #[must_use]
    pub const fn tool_calls(self) -> ToolCallCount {
        self.tool_calls
    }
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.duration.as_micros() == 0
            && self.tokens.value() == 0
            && self.cost.value() == 0
            && self.tool_calls.value() == 0
    }
}
