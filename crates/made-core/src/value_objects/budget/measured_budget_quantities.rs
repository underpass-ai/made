use super::{BudgetMeasurement, BudgetTokenCount, CostMicros, ExecutionDuration, ToolCallCount};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredBudgetQuantities {
    duration: BudgetMeasurement<ExecutionDuration>,
    tokens: BudgetMeasurement<BudgetTokenCount>,
    cost: BudgetMeasurement<CostMicros>,
    tool_calls: BudgetMeasurement<ToolCallCount>,
}
impl MeasuredBudgetQuantities {
    #[must_use]
    pub const fn new(
        duration: BudgetMeasurement<ExecutionDuration>,
        tokens: BudgetMeasurement<BudgetTokenCount>,
        cost: BudgetMeasurement<CostMicros>,
        tool_calls: BudgetMeasurement<ToolCallCount>,
    ) -> Self {
        Self {
            duration,
            tokens,
            cost,
            tool_calls,
        }
    }
    #[must_use]
    pub const fn duration(self) -> BudgetMeasurement<ExecutionDuration> {
        self.duration
    }
    #[must_use]
    pub const fn tokens(self) -> BudgetMeasurement<BudgetTokenCount> {
        self.tokens
    }
    #[must_use]
    pub const fn cost(self) -> BudgetMeasurement<CostMicros> {
        self.cost
    }
    #[must_use]
    pub const fn tool_calls(self) -> BudgetMeasurement<ToolCallCount> {
        self.tool_calls
    }

    #[must_use]
    pub fn advances(self, previous: Self) -> bool {
        let comparisons = [
            quality(&self.duration).cmp(&quality(&previous.duration)),
            quality(&self.tokens).cmp(&quality(&previous.tokens)),
            quality(&self.cost).cmp(&quality(&previous.cost)),
            quality(&self.tool_calls).cmp(&quality(&previous.tool_calls)),
        ];
        comparisons.iter().all(|order| !order.is_lt())
            && comparisons.iter().any(|order| order.is_gt())
    }

    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self.duration, BudgetMeasurement::Unknown)
            && matches!(self.tokens, BudgetMeasurement::Unknown)
            && matches!(self.cost, BudgetMeasurement::Unknown)
            && matches!(self.tool_calls, BudgetMeasurement::Unknown)
    }
}

fn quality<T>(measurement: &BudgetMeasurement<T>) -> u8 {
    match measurement {
        BudgetMeasurement::Unknown => 0,
        BudgetMeasurement::Estimated(_) => 1,
        BudgetMeasurement::Observed(_) => 2,
    }
}
