use made_core::value_objects::{BudgetLimits, CeremonyId};

use crate::usecases::StartCeremonyInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartBudgetedCeremonyInput {
    pub(crate) ceremony: StartCeremonyInput,
    pub(crate) limits: BudgetLimits,
}

impl StartBudgetedCeremonyInput {
    #[must_use]
    pub const fn new(ceremony: StartCeremonyInput, limits: BudgetLimits) -> Self {
        Self { ceremony, limits }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        self.ceremony.id()
    }
}
