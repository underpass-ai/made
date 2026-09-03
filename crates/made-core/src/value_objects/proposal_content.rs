use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Provider-authored proposal text.
///
/// Non-emptiness is an aggregate invariant enforced by `Proposal`; this value
/// object gives the port boundary a semantic type before aggregate creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProposalContent(String);

impl ProposalContent {
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProposalContent {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ProposalContent {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl Deref for ProposalContent {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for ProposalContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for ProposalContent {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ProposalContent {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
