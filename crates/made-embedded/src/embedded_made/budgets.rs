use made_core::ports::BudgetReservationPage;
use made_core::value_objects::{
    BudgetAccountId, BudgetBalance, BudgetPageLimit, BudgetReservationId,
};
use made_core::BudgetError;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn budget_report(
        &self,
        account: &BudgetAccountId,
    ) -> Result<BudgetBalance, BudgetError> {
        self.budgets.report(account).await
    }

    pub async fn pending_budget_reservations(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, BudgetError> {
        self.budgets.pending(after, limit).await
    }
}
