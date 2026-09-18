use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::DomainError;
use crate::value_objects::ceremony::{
    CeremonyId, StateIteration, StateVisit, StepId, StepIteration,
};

const SCHEME: &[u8] = b"made.execution-operation.v1\0";

/// Stable identity of one semantic external operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionOperationId(String);

impl ExecutionOperationId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidCharacters {
                field: "execution_operation_id",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn for_step(
        ceremony_id: &CeremonyId,
        step_id: &StepId,
        state_visit: StateVisit,
        state_iteration: StateIteration,
        step_iteration: StepIteration,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(SCHEME);
        for part in [
            ceremony_id.as_str().as_bytes().to_vec(),
            step_id.as_str().as_bytes().to_vec(),
            state_visit.get().to_be_bytes().to_vec(),
            state_iteration.get().to_be_bytes().to_vec(),
            step_iteration.get().to_be_bytes().to_vec(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        Self(format!("{:x}", digest.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExecutionOperationId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionOperationId> for String {
    fn from(value: ExecutionOperationId) -> Self {
        value.0
    }
}

impl fmt::Display for ExecutionOperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
