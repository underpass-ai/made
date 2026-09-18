use crate::{value_objects::ExecutionReceiptId, DomainError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetReconciliationId(String);

impl BudgetReconciliationId {
    #[must_use]
    pub fn for_receipt(receipt_id: &ExecutionReceiptId) -> Self {
        Self(receipt_id.as_str().to_owned())
    }

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "budget_reconciliation_id",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
