use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetQuantities {
    pub duration_micros: u64,
    pub tokens: u64,
    pub cost_micros: u64,
    pub tool_calls: u64,
}
