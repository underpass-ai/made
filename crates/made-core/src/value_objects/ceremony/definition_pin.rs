use std::fmt;

use serde::{Deserialize, Serialize};

use super::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};

/// An immutable reference to one published ceremony definition.
///
/// Name and version say which definition; the digest says which bytes.
/// A composition that pinned only the version would follow a republish
/// it never agreed to, so the digest is part of the reference rather
/// than a check performed beside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DefinitionPin {
    name: CeremonyName,
    version: CeremonyVersion,
    digest: CeremonyDefinitionDigest,
}

impl DefinitionPin {
    #[must_use]
    pub const fn new(
        name: CeremonyName,
        version: CeremonyVersion,
        digest: CeremonyDefinitionDigest,
    ) -> Self {
        Self {
            name,
            version,
            digest,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &CeremonyVersion {
        &self.version
    }

    #[must_use]
    pub const fn digest(&self) -> CeremonyDefinitionDigest {
        self.digest
    }

    /// Whether this pin names the same bytes as another.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Display for DefinitionPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.name, self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(version: &str, fill: u8) -> DefinitionPin {
        DefinitionPin::new(
            CeremonyName::new("review").unwrap(),
            CeremonyVersion::new(version).unwrap(),
            CeremonyDefinitionDigest::from_bytes([fill; 32]),
        )
    }

    #[test]
    fn a_republished_version_is_not_the_pin_that_was_agreed_to() {
        assert!(pin("1.0.0", 0x0a).matches(&pin("1.0.0", 0x0a)));
        assert!(!pin("1.0.0", 0x0a).matches(&pin("1.0.0", 0x0b)));
        assert_eq!(pin("1.0.0", 0x0a).to_string(), "review@1.0.0");
    }
}
