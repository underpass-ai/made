use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Provenance class for a reported consumption value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentUsageKind {
    Measured,
    Estimated,
    Unavailable,
}

impl TryFrom<&str> for AgentUsageKind {
    type Error = DomainError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "measured" => Ok(Self::Measured),
            "estimated" => Ok(Self::Estimated),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(DomainError::InvariantViolated {
                reason: "unsupported agent usage kind",
            }),
        }
    }
}
