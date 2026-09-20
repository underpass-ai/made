use std::fmt;

use serde::{Deserialize, Serialize};

/// Whether a logical participant is meant to be a person or an agent.
///
/// A design states which it wants; the host decides what actually
/// turns up, and a person where an agent was designed for is a fact
/// the run records rather than a validation error here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantKind {
    Person,
    Agent,
}

impl ParticipantKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for ParticipantKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_kinds_name_themselves() {
        assert_eq!(ParticipantKind::Person.to_string(), "person");
        assert_eq!(ParticipantKind::Agent.to_string(), "agent");
    }
}
