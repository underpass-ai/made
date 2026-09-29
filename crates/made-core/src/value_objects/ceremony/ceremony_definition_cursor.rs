use std::fmt;

use crate::error::DomainError;

use super::{CeremonyName, CeremonyVersion};

/// The separator between name and version in a written cursor.
///
/// Neither half can contain it: a name is lowercase ASCII, digits and
/// underscores, and a version is ASCII alphanumerics, `.`, `-` and `_`.
/// So the written form needs no escaping and splits exactly one way.
const SEPARATOR: char = '@';

/// Where a page of the published catalogue ends: the name and version
/// of the last definition it returned.
///
/// Not an opaque token. The catalogue is ordered by name and then by
/// version, the order is public, and a token that only re-encoded it
/// would be one more thing to keep true — the same choice the agentic
/// system catalogue made for its identifiers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyDefinitionCursor {
    name: CeremonyName,
    version: CeremonyVersion,
}

impl CeremonyDefinitionCursor {
    #[must_use]
    pub const fn new(name: CeremonyName, version: CeremonyVersion) -> Self {
        Self { name, version }
    }

    /// Read a cursor in the form `name@version` a previous page wrote.
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        // Without the separator the cursor names nothing: the caller's
        // to fix, like any other malformed identifier.
        let (name, version) = raw
            .split_once(SEPARATOR)
            .ok_or(DomainError::InvalidCharacters {
                field: "ceremony_definition_cursor",
            })?;
        Ok(Self::new(
            CeremonyName::new(name)?,
            CeremonyVersion::new(version)?,
        ))
    }

    #[must_use]
    pub const fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &CeremonyVersion {
        &self.version
    }

    /// Whether the definition at `name` and `version` comes after this
    /// cursor in catalogue order, and so belongs to a later page.
    #[must_use]
    pub fn precedes(&self, name: &CeremonyName, version: &CeremonyVersion) -> bool {
        (&self.name, &self.version) < (name, version)
    }
}

impl fmt::Display for CeremonyDefinitionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{SEPARATOR}{}", self.name, self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> CeremonyName {
        CeremonyName::new(raw).unwrap()
    }

    fn version(raw: &str) -> CeremonyVersion {
        CeremonyVersion::new(raw).unwrap()
    }

    #[test]
    fn the_written_form_reads_back_as_the_same_cursor() {
        let cursor = CeremonyDefinitionCursor::new(name("pr_review"), version("1.0-rc_2"));
        let written = cursor.to_string();

        assert_eq!(written, "pr_review@1.0-rc_2");
        assert_eq!(CeremonyDefinitionCursor::parse(&written).unwrap(), cursor);
        assert_eq!(cursor.name().as_str(), "pr_review");
        assert_eq!(cursor.version().as_str(), "1.0-rc_2");
    }

    #[test]
    fn a_cursor_without_both_halves_is_refused() {
        assert!(CeremonyDefinitionCursor::parse("pr_review").is_err());
        assert!(CeremonyDefinitionCursor::parse("@1.0").is_err());
        assert!(CeremonyDefinitionCursor::parse("pr_review@").is_err());
        assert!(CeremonyDefinitionCursor::parse("pr_review@1.0@2").is_err());
    }

    #[test]
    fn a_cursor_excludes_everything_up_to_and_including_itself() {
        let cursor = CeremonyDefinitionCursor::new(name("b"), version("2.0"));

        assert!(!cursor.precedes(&name("a"), &version("9.0")));
        assert!(!cursor.precedes(&name("b"), &version("1.0")));
        assert!(!cursor.precedes(&name("b"), &version("2.0")));
        assert!(cursor.precedes(&name("b"), &version("3.0")));
        assert!(cursor.precedes(&name("c"), &version("0.1")));
    }
}
