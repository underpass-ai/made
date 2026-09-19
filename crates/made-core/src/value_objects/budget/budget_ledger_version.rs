use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetLedgerVersion(u64);

impl BudgetLedgerVersion {
    pub const EMPTY: Self = Self(0);
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
