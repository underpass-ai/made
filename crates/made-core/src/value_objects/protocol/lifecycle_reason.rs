use crate::error::DomainError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LifecycleReason(String);

impl LifecycleReason {
    pub const MAX_LEN: usize = 1000;
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "lifecycle_reason",
            });
        }
        if value.len() > Self::MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "lifecycle_reason",
                actual: value.len(),
                max: Self::MAX_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for LifecycleReason {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<LifecycleReason> for String {
    fn from(value: LifecycleReason) -> Self {
        value.0
    }
}
