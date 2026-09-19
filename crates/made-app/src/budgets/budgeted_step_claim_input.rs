use made_core::value_objects::{BudgetReservationEstimate, CeremonyId};

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

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        self.claim.instance_id()
    }
}
