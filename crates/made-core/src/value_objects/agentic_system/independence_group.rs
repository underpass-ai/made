use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::slug;

const MAX_LEN: usize = 128;

/// A set of participants that count as one party when independence is
/// judged.
///
/// Two agents on the same model with the same prompt are one opinion
/// wearing two names; declaring the group is how a design says so.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct IndependenceGroup(String);

impl IndependenceGroup {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        slug::slug("independence_group", raw.as_ref(), MAX_LEN).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for IndependenceGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<&str> for IndependenceGroup {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for IndependenceGroup {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Decoding goes through the constructor.
///
/// A derived implementation would accept a stored or transmitted value
/// this type refuses to be built from, and the invariant would hold
/// everywhere except where the document came from outside — which is
/// the only place it was ever at risk.
impl<'de> Deserialize<'de> for IndependenceGroup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
