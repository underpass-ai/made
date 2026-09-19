use async_trait::async_trait;

use crate::value_objects::{BudgetReservationEstimate, BudgetReservationRequest};
use crate::DomainError;

/// Host or handler policy that supplies bounded pre-execution quantities without defaults.
#[async_trait]
pub trait BudgetReservationPlannerPort: Send + Sync {
    async fn estimate(
        &self,
        request: &BudgetReservationRequest,
    ) -> Result<BudgetReservationEstimate, DomainError>;
}
