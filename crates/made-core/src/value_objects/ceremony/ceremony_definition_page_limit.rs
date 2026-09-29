use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const DEFAULT_LIMIT: u16 = 50;
const MAX_LIMIT: u16 = 100;

/// How many published definitions one page of the catalogue holds.
///
/// Bounded at the type rather than at each adapter, so a listing cannot
/// be made unbounded by a caller sending a large number and an adapter
/// forgetting to clamp it. The same bounds every other public listing
/// uses: fifty by default, never more than a hundred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CeremonyDefinitionPageLimit(u16);

impl CeremonyDefinitionPageLimit {
    pub const MAX: u16 = MAX_LIMIT;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 || value > MAX_LIMIT {
            return Err(DomainError::OutOfRange {
                field: "ceremony_definition_page_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(MAX_LIMIT),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Default for CeremonyDefinitionPageLimit {
    fn default() -> Self {
        Self(DEFAULT_LIMIT)
    }
}

impl fmt::Display for CeremonyDefinitionPageLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for CeremonyDefinitionPageLimit {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u16::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_at_least_one_and_never_unbounded() {
        assert_eq!(CeremonyDefinitionPageLimit::default().get(), DEFAULT_LIMIT);
        assert_eq!(
            CeremonyDefinitionPageLimit::new(100).unwrap().as_usize(),
            100
        );
        assert!(CeremonyDefinitionPageLimit::new(0).is_err());
        assert!(CeremonyDefinitionPageLimit::new(MAX_LIMIT + 1).is_err());
        assert_eq!(
            CeremonyDefinitionPageLimit::new(7).unwrap().to_string(),
            "7"
        );
    }

    #[test]
    fn deserializing_applies_the_same_bounds() {
        assert!(serde_json::from_str::<CeremonyDefinitionPageLimit>("0").is_err());
        assert_eq!(
            serde_json::from_str::<CeremonyDefinitionPageLimit>("3").unwrap(),
            CeremonyDefinitionPageLimit::new(3).unwrap()
        );
    }
}
