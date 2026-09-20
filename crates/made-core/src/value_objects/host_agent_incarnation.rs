use crate::error::DomainError;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
const MAX_LEN: usize = 256;
/// Opaque generation of a host executor; changes when that process is replaced.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct HostAgentIncarnation(String);
impl HostAgentIncarnation {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "host_agent_incarnation",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "host_agent_incarnation",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "host_agent_incarnation",
            });
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for HostAgentIncarnation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// This is deliberately not a derived transparent deserialisation: a derived
// `String` wrapper could reconstitute an invalid incarnation from an embedded
// JSON/MCP payload and bypass the constructor used by protocol boundaries.
impl<'de> Deserialize<'de> for HostAgentIncarnation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::HostAgentIncarnation;

    #[test]
    fn serde_reuses_constructor_validation_and_normalisation() {
        let valid: HostAgentIncarnation = serde_json::from_str("\"  host-run-7  \"").unwrap();
        assert_eq!(valid.as_str(), "host-run-7");

        for invalid in ["\"\"", "\" \\t \"", "\"host\\u0000run\""] {
            assert!(
                serde_json::from_str::<HostAgentIncarnation>(invalid).is_err(),
                "{invalid}"
            );
        }
        let too_long = format!("\"{}\"", "x".repeat(257));
        assert!(serde_json::from_str::<HostAgentIncarnation>(&too_long).is_err());
    }
}
