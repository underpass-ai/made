use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::slug;

const MAX_LEN: usize = 128;

/// The name one composition goes by inside its system.
///
/// Distinct from the pinned ceremony name, because a system may
/// compose the same definition twice for two different purposes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SystemCeremonyId(String);

impl SystemCeremonyId {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        slug::slug("system_ceremony_id", raw.as_ref(), MAX_LEN).map(Self)
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

impl fmt::Display for SystemCeremonyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<&str> for SystemCeremonyId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for SystemCeremonyId {
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
impl<'de> Deserialize<'de> for SystemCeremonyId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
