use serde::{Deserialize, Serialize};

use crate::BudgetQuantities;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReservation {
    pub reservation_id: String,
    pub operation_id: String,
    pub quantities: BudgetQuantities,
    pub reserved_at_millis: i64,
    pub reconciled: bool,
}
