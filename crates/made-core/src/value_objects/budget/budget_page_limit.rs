use crate::DomainError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetPageLimit(usize);
impl BudgetPageLimit {
    pub fn new(value: usize) -> Result<Self, DomainError> {
        if value == 0 || value > 500 {
            return Err(DomainError::OutOfRange {
                field: "budget_page_limit",
                value: value as f64,
                min: 1.0,
                max: 500.0,
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}
