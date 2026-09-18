use async_trait::async_trait;

use crate::entities::BudgetLedgerEvent;
use crate::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetPageLimit, BudgetReservationId,
};
use crate::DomainError;

use super::{BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetReservationPage};

#[async_trait]
pub trait BudgetLedgerStorePort: Send + Sync {
    async fn load(
        &self,
        account: &BudgetAccountId,
    ) -> Result<Option<BudgetLedgerSnapshot>, DomainError>;
    async fn append(
        &self,
        account: &BudgetAccountId,
        expected: BudgetLedgerVersion,
        events: Vec<BudgetLedgerEvent>,
    ) -> Result<BudgetAppendOutcome, DomainError>;
    async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, DomainError>;
}
