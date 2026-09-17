use std::fmt;

use crate::error::DomainError;

/// A Prometheus metric or sample name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricName(String);

impl MetricName {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let mut chars = value.chars();
        let valid_first = chars.next().is_some_and(|character| {
            character.is_ascii_alphabetic() || matches!(character, '_' | ':')
        });
        if !valid_first
            || !chars.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | ':')
            })
        {
            return Err(DomainError::InvariantViolated {
                reason: "metric name must use the Prometheus identifier alphabet",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MetricName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
