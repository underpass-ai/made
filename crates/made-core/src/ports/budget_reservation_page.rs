use crate::value_objects::BudgetReservation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReservationPage {
    reservations: Vec<BudgetReservation>,
}

impl BudgetReservationPage {
    #[must_use]
    pub const fn new(reservations: Vec<BudgetReservation>) -> Self {
        Self { reservations }
    }
    #[must_use]
    pub fn reservations(&self) -> &[BudgetReservation] {
        &self.reservations
    }
}
