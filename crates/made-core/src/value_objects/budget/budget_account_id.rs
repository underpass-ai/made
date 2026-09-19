use serde::{Deserialize, Serialize};

use crate::{value_objects::CeremonyId, DomainError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetAccountId(String);

impl BudgetAccountId {
    pub fn for_root(root: &CeremonyId) -> Result<Self, DomainError> {
        Self::new(root.as_str())
    }

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "budget_account_id",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
