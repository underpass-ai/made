use serde::{Deserialize, Serialize};

use crate::{BudgetLimits, BudgetQuantities};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetBalance {
    pub limits: BudgetLimits,
    pub reserved: BudgetQuantities,
    pub observed: BudgetQuantities,
    pub estimated: BudgetQuantities,
    pub unconfirmed: BudgetQuantities,
    pub overrun: BudgetQuantities,
    pub available: BudgetQuantities,
}
