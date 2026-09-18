use thiserror::Error;

use crate::value_objects::{
    BudgetAccountId, BudgetDimension, BudgetReconciliationId, BudgetReservationId,
};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum BudgetError {
    #[error("budget {account_id:?} is exhausted for {dimension:?}")]
    Exhausted {
        account_id: BudgetAccountId,
        dimension: BudgetDimension,
    },
    #[error("budget ledger is not open")]
    LedgerNotOpen,
    #[error("budget reservation not found: {0:?}")]
    ReservationNotFound(BudgetReservationId),
    #[error("budget reservation conflicts with stored identity: {0:?}")]
    ReservationConflict(BudgetReservationId),
    #[error("budget reconciliation conflicts with stored observation: {0:?}")]
    ReconciliationConflict(BudgetReconciliationId),
    #[error("budget persistence failed: {0}")]
    Persistence(DomainError),
}

impl From<DomainError> for BudgetError {
    fn from(value: DomainError) -> Self {
        Self::Persistence(value)
    }
}
