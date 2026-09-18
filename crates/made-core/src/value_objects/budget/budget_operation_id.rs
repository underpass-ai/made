use crate::value_objects::ExecutionOperationId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BudgetOperationId(String);

impl BudgetOperationId {
    #[must_use]
    pub fn for_execution(operation_id: &ExecutionOperationId) -> Self {
        Self(operation_id.as_str().to_owned())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BudgetOperationId {
    type Error = crate::DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ExecutionOperationId::new(value).map(|operation| Self::for_execution(&operation))
    }
}

impl From<BudgetOperationId> for String {
    fn from(operation_id: BudgetOperationId) -> Self {
        operation_id.0
    }
}
