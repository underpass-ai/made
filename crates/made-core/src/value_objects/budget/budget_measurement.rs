use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "quality", content = "amount")]
pub enum BudgetMeasurement<T> {
    Observed(T),
    Estimated(T),
    #[default]
    Unknown,
}
