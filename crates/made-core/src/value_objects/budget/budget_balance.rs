use super::{BudgetLimits, BudgetQuantities};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetBalance {
    limits: BudgetLimits,
    reserved: BudgetQuantities,
    observed: BudgetQuantities,
    estimated: BudgetQuantities,
    unconfirmed: BudgetQuantities,
    overrun: BudgetQuantities,
    available: BudgetQuantities,
}
impl BudgetBalance {
    #[must_use]
    pub const fn new(
        limits: BudgetLimits,
        reserved: BudgetQuantities,
        observed: BudgetQuantities,
        estimated: BudgetQuantities,
        unconfirmed: BudgetQuantities,
        overrun: BudgetQuantities,
        available: BudgetQuantities,
    ) -> Self {
        Self {
            limits,
            reserved,
            observed,
            estimated,
            unconfirmed,
            overrun,
            available,
        }
    }
    #[must_use]
    pub const fn limits(&self) -> &BudgetLimits {
        &self.limits
    }
    #[must_use]
    pub const fn reserved(&self) -> BudgetQuantities {
        self.reserved
    }
    #[must_use]
    pub const fn observed(&self) -> BudgetQuantities {
        self.observed
    }
    #[must_use]
    pub const fn estimated(&self) -> BudgetQuantities {
        self.estimated
    }
    #[must_use]
    pub const fn unconfirmed(&self) -> BudgetQuantities {
        self.unconfirmed
    }
    #[must_use]
    pub const fn overrun(&self) -> BudgetQuantities {
        self.overrun
    }
    #[must_use]
    pub const fn available(&self) -> BudgetQuantities {
        self.available
    }
}
