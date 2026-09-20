use std::fmt;

use serde::{Deserialize, Serialize};

/// How far along an agentic system design is.
///
/// A draft may be edited freely; publishing seals a revision so a run
/// can name what it composed. Deprecation says "do not start new runs
/// from this" without invalidating the ones already sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticSystemLifecycle {
    Draft,
    Published,
    Deprecated,
}

impl AgenticSystemLifecycle {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Deprecated => "deprecated",
        }
    }

    /// Whether a run may be instantiated from a design in this state.
    #[must_use]
    pub const fn is_instantiable(self) -> bool {
        matches!(self, Self::Published)
    }
}

impl fmt::Display for AgenticSystemLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_published_design_can_be_run() {
        assert!(AgenticSystemLifecycle::Published.is_instantiable());
        assert!(!AgenticSystemLifecycle::Draft.is_instantiable());
        assert!(!AgenticSystemLifecycle::Deprecated.is_instantiable());
        assert_eq!(AgenticSystemLifecycle::Draft.to_string(), "draft");
    }
}
