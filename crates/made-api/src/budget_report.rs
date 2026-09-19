use serde::{Deserialize, Serialize};

use crate::BudgetBalance;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReport {
    pub account_id: String,
    pub balance: BudgetBalance,
}
