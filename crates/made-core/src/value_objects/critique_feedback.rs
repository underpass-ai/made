use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Peer-review feedback returned by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CritiqueFeedback(String);

impl CritiqueFeedback {
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for CritiqueFeedback {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for CritiqueFeedback {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl Deref for CritiqueFeedback {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for CritiqueFeedback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for CritiqueFeedback {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for CritiqueFeedback {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
