use super::{BudgetAccountId, BudgetOperationId};
use crate::DomainError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetReservationId(String);

impl BudgetReservationId {
    #[must_use]
    pub fn for_operation(account: &BudgetAccountId, operation: &BudgetOperationId) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"made:budget-reservation:v1\0");
        digest.update(account.as_str().as_bytes());
        digest.update(b"\0");
        digest.update(operation.as_str().as_bytes());
        Self(format!("{:x}", digest.finalize()))
    }
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "budget_reservation_id",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
