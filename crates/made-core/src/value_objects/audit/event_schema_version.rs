//! [`EventSchemaVersion`] — the shape a ceremony event's payload was
//! written in.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Version of a ceremony event's payload shape.
///
/// Every event type versions its payload on its own, apart from the
/// record envelope around it: adding, renaming or reinterpreting a
/// field is a new version with a reader for the old one, never an
/// in-place edit. The version is sealed into the record's digest, so a
/// payload cannot be reread under a shape it was not written in
/// without the chain noticing.
///
/// On the wire it is the bare number; zero is refused on the way in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct EventSchemaVersion(u32);

impl EventSchemaVersion {
    /// The first payload shape of every event type, and today the only
    /// one.
    pub const V1: Self = Self(1);
    pub const V2: Self = Self(2);
    pub const V3: Self = Self(3);
    pub const V4: Self = Self(4);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "event_schema_version",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for EventSchemaVersion {
    type Error = DomainError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<EventSchemaVersion> for u32 {
    fn from(version: EventSchemaVersion) -> Self {
        version.0
    }
}

impl fmt::Display for EventSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_one_is_the_first_shape() {
        assert_eq!(EventSchemaVersion::V1.get(), 1);
        assert_eq!(EventSchemaVersion::new(1).unwrap(), EventSchemaVersion::V1);
        assert_eq!(EventSchemaVersion::V1.to_string(), "1");
    }

    #[test]
    fn zero_is_refused_in_code_and_on_the_wire() {
        assert!(matches!(
            EventSchemaVersion::new(0),
            Err(DomainError::MustBeNonZero { .. })
        ));
        assert!(serde_json::from_str::<EventSchemaVersion>("0").is_err());
    }

    #[test]
    fn it_travels_as_a_bare_number() {
        let version = EventSchemaVersion::new(3).unwrap();

        assert_eq!(serde_json::to_string(&version).unwrap(), "3");
        assert_eq!(
            serde_json::from_str::<EventSchemaVersion>("3").unwrap(),
            version
        );
    }
}
