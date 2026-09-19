use serde::{Deserialize, Serialize};

use crate::DomainError;

const MAX: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuthorizationDecisionPageLimit(usize);

impl AuthorizationDecisionPageLimit {
    pub fn new(value: usize) -> Result<Self, DomainError> {
        if value == 0 || value > MAX {
            return Err(DomainError::OutOfRange {
                field: "authorization_decision_page_limit",
                value: value as f64,
                min: 1.0,
                max: MAX as f64,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}
