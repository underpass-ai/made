use made_core::value_objects::BudgetReservationEstimate;

use crate::usecases::StartCeremonyStepInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetedStepClaimInput {
    pub(crate) claim: StartCeremonyStepInput,
    pub(crate) reservation: BudgetReservationEstimate,
}

impl BudgetedStepClaimInput {
    #[must_use]
    pub const fn new(
        claim: StartCeremonyStepInput,
        reservation: BudgetReservationEstimate,
    ) -> Self {
        Self { claim, reservation }
    }
}
