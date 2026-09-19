use serde::{Deserialize, Serialize};

/// Optional positive ceilings for a ceremony tree. At least one must be set.
/// An explicit zero is rejected; only `None` leaves a dimension unlimited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetLimits {
    pub duration_micros: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_micros: Option<u64>,
    pub tool_calls: Option<u64>,
    pub currency: Option<String>,
}
