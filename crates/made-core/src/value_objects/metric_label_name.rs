use crate::error::DomainError;

/// A Prometheus label name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricLabelName(String);

impl MetricLabelName {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let mut chars = value.chars();
        let valid_first = chars
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_');
        if !valid_first
            || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(DomainError::InvariantViolated {
                reason: "metric label name must use the Prometheus label alphabet",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
