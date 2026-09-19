use made_core::value_objects::{BudgetAccountId, BudgetReservationId, ExecutionOperationId};

use crate::usecases::StartCeremonyStepOutput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetedStepClaimOutput {
    claim: StartCeremonyStepOutput,
    account_id: BudgetAccountId,
    operation_id: ExecutionOperationId,
    reservation_id: BudgetReservationId,
}

impl BudgetedStepClaimOutput {
    #[must_use]
    pub const fn new(
        claim: StartCeremonyStepOutput,
        account_id: BudgetAccountId,
        operation_id: ExecutionOperationId,
        reservation_id: BudgetReservationId,
    ) -> Self {
        Self {
            claim,
            account_id,
            operation_id,
            reservation_id,
        }
    }

    #[must_use]
    pub const fn claim(&self) -> &StartCeremonyStepOutput {
        &self.claim
    }

    #[must_use]
    pub const fn account_id(&self) -> &BudgetAccountId {
        &self.account_id
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn reservation_id(&self) -> &BudgetReservationId {
        &self.reservation_id
    }
}
