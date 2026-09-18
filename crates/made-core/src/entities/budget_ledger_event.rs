use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    BudgetAccountId, BudgetLimits, BudgetOperationId, BudgetQuantities, BudgetReconciliationId,
    BudgetReservationEstimate, BudgetReservationId, MeasuredBudgetQuantities,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum BudgetLedgerEvent {
    Opened {
        account_id: BudgetAccountId,
        limits: BudgetLimits,
        #[serde(with = "time::serde::rfc3339")]
        opened_at: OffsetDateTime,
    },
    Reserved {
        account_id: BudgetAccountId,
        reservation_id: BudgetReservationId,
        operation_id: BudgetOperationId,
        quantities: BudgetQuantities,
        estimate: BudgetReservationEstimate,
        #[serde(with = "time::serde::rfc3339")]
        reserved_at: OffsetDateTime,
    },
    Reconciled {
        account_id: BudgetAccountId,
        reservation_id: BudgetReservationId,
        reconciliation_id: BudgetReconciliationId,
        measured: MeasuredBudgetQuantities,
        #[serde(with = "time::serde::rfc3339")]
        reconciled_at: OffsetDateTime,
    },
}

impl BudgetLedgerEvent {
    #[must_use]
    pub const fn account_id(&self) -> &BudgetAccountId {
        match self {
            Self::Opened { account_id, .. }
            | Self::Reserved { account_id, .. }
            | Self::Reconciled { account_id, .. } => account_id,
        }
    }
}
