use crate::DomainError;

/// Version of the host's declared reservation-estimation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetReservationPolicyVersion(u64);

impl BudgetReservationPolicyVersion {
    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "budget_reservation_policy_version",
            });
        }
        Ok(Self(value))
    }
}
