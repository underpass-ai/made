use super::{
    BudgetOperationId, BudgetQuantities, BudgetReconciliationId, BudgetReservationId,
    MeasuredBudgetQuantities,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReservation {
    id: BudgetReservationId,
    operation_id: BudgetOperationId,
    quantities: BudgetQuantities,
    #[serde(with = "time::serde::rfc3339")]
    reserved_at: OffsetDateTime,
    reconciliation: Option<(BudgetReconciliationId, MeasuredBudgetQuantities)>,
}
impl BudgetReservation {
    #[must_use]
    pub const fn new(
        id: BudgetReservationId,
        operation_id: BudgetOperationId,
        quantities: BudgetQuantities,
        reserved_at: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            operation_id,
            quantities,
            reserved_at,
            reconciliation: None,
        }
    }
    #[must_use]
    pub fn id(&self) -> &BudgetReservationId {
        &self.id
    }
    #[must_use]
    pub fn operation_id(&self) -> &BudgetOperationId {
        &self.operation_id
    }
    #[must_use]
    pub const fn quantities(&self) -> BudgetQuantities {
        self.quantities
    }
    #[must_use]
    pub const fn reserved_at(&self) -> OffsetDateTime {
        self.reserved_at
    }
    #[must_use]
    pub const fn reconciliation(
        &self,
    ) -> Option<&(BudgetReconciliationId, MeasuredBudgetQuantities)> {
        self.reconciliation.as_ref()
    }
    pub(crate) fn reconcile(
        &mut self,
        id: BudgetReconciliationId,
        measured: MeasuredBudgetQuantities,
    ) {
        self.reconciliation = Some((id, measured));
    }
}
