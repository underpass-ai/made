use made_core::value_objects::{BudgetReservationEstimate, ProtocolId};

use crate::usecases::StartProtocolStepInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetedStepClaimInput {
    pub(crate) claim: StartProtocolStepInput,
    pub(crate) reservation: BudgetReservationEstimate,
}

impl BudgetedStepClaimInput {
    #[must_use]
    pub const fn new(
        claim: StartProtocolStepInput,
        reservation: BudgetReservationEstimate,
    ) -> Self {
        Self { claim, reservation }
    }

    #[must_use]
    pub fn protocol_id(&self) -> &ProtocolId {
        self.claim.instance_id()
    }
}
