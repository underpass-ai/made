use serde::{Deserialize, Serialize};

use super::{
    BudgetDimension, BudgetLimits, BudgetMeasurement, BudgetQuantities, BudgetTokenCount,
    CostMicros, ExecutionDuration, ToolCallCount,
};
use crate::BudgetError;

/// Pre-execution reservation plan supplied by a host or handler.
///
/// `Estimated` and `Observed` describe how the bound was obtained before execution; they are
/// distinct from the terminal consumption later carried by an execution receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReservationEstimate {
    duration: BudgetMeasurement<ExecutionDuration>,
    tokens: BudgetMeasurement<BudgetTokenCount>,
    cost: BudgetMeasurement<CostMicros>,
    tool_calls: BudgetMeasurement<ToolCallCount>,
}

impl BudgetReservationEstimate {
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

    pub fn quantities(self, limits: &BudgetLimits) -> Result<BudgetQuantities, BudgetError> {
        Ok(BudgetQuantities::new(
            duration(self.duration, limits.duration().is_some())?,
            tokens(self.tokens, limits.tokens().is_some())?,
            cost(self.cost, limits.cost().is_some())?,
            tool_calls(self.tool_calls, limits.tool_calls().is_some())?,
        ))
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
}

fn duration(
    value: BudgetMeasurement<ExecutionDuration>,
    limited: bool,
) -> Result<ExecutionDuration, BudgetError> {
    match value {
        BudgetMeasurement::Observed(value) | BudgetMeasurement::Estimated(value) => Ok(value),
        BudgetMeasurement::Unknown if !limited => Ok(ExecutionDuration::default()),
        BudgetMeasurement::Unknown => Err(BudgetError::MissingReservationEstimate(
            BudgetDimension::Duration,
        )),
    }
}

fn tokens(
    value: BudgetMeasurement<BudgetTokenCount>,
    limited: bool,
) -> Result<BudgetTokenCount, BudgetError> {
    match value {
        BudgetMeasurement::Observed(value) | BudgetMeasurement::Estimated(value) => Ok(value),
        BudgetMeasurement::Unknown if !limited => Ok(BudgetTokenCount::default()),
        BudgetMeasurement::Unknown => Err(BudgetError::MissingReservationEstimate(
            BudgetDimension::Tokens,
        )),
    }
}

fn cost(value: BudgetMeasurement<CostMicros>, limited: bool) -> Result<CostMicros, BudgetError> {
    match value {
        BudgetMeasurement::Observed(value) | BudgetMeasurement::Estimated(value) => Ok(value),
        BudgetMeasurement::Unknown if !limited => Ok(CostMicros::default()),
        BudgetMeasurement::Unknown => Err(BudgetError::MissingReservationEstimate(
            BudgetDimension::Cost,
        )),
    }
}

fn tool_calls(
    value: BudgetMeasurement<ToolCallCount>,
    limited: bool,
) -> Result<ToolCallCount, BudgetError> {
    match value {
        BudgetMeasurement::Observed(value) | BudgetMeasurement::Estimated(value) => Ok(value),
        BudgetMeasurement::Unknown if !limited => Ok(ToolCallCount::default()),
        BudgetMeasurement::Unknown => Err(BudgetError::MissingReservationEstimate(
            BudgetDimension::ToolCalls,
        )),
    }
}
