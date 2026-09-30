//! [`CeremonyDefinitionDigestScheme`] — the domain separator a
//! publication was sealed under.

/// A version of the ceremony-definition digest algorithm.
///
/// The algorithm is SHA-256 over a domain separator followed by the
/// canonical JSON of the definition; the separator is what versions it.
/// New publications are always sealed under [`Self::CURRENT`]. The
/// other variants exist only so a publication sealed before a change of
/// separator can still be *verified* under the separator it was sealed
/// with: the stored bytes are never rewritten, and a digest that matches
/// none of these schemes is still refused.
///
/// The set is closed on purpose. Accepting "whatever the stored digest
/// happens to be" would turn content identity into a name-based claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CeremonyDefinitionDigestScheme {
    /// `underpass.made.ceremony-definition.v1`, the scheme since the
    /// rename to MADE (v0.1.0).
    MadeV1,
    /// `underpass.choreo.ceremony-definition.v1`, the scheme of every
    /// publication sealed by Underpass Choreographer before the rename.
    /// Stores carried across the rename without the ADR-008 importer
    /// still hold rows sealed this way (issue #253).
    ChoreographerV1,
}

impl CeremonyDefinitionDigestScheme {
    /// The scheme every new publication is sealed under.
    pub const CURRENT: Self = Self::MadeV1;

    /// Every scheme a stored publication may be verified under, the
    /// current one first.
    pub const KNOWN: [Self; 2] = [Self::MadeV1, Self::ChoreographerV1];

    /// The domain separator hashed before the canonical form.
    #[must_use]
    pub const fn domain_separator(self) -> &'static [u8] {
        match self {
            Self::MadeV1 => b"underpass.made.ceremony-definition.v1",
            Self::ChoreographerV1 => b"underpass.choreo.ceremony-definition.v1",
        }
    }

    /// The domain separator as text, for operators and audit output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MadeV1 => "underpass.made.ceremony-definition.v1",
            Self::ChoreographerV1 => "underpass.choreo.ceremony-definition.v1",
        }
    }

    #[must_use]
    pub const fn is_current(self) -> bool {
        matches!(self, Self::MadeV1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_scheme_is_known_and_first() {
        assert_eq!(
            CeremonyDefinitionDigestScheme::KNOWN[0],
            CeremonyDefinitionDigestScheme::CURRENT
        );
        assert!(CeremonyDefinitionDigestScheme::CURRENT.is_current());
        assert!(!CeremonyDefinitionDigestScheme::ChoreographerV1.is_current());
    }

    #[test]
    fn text_and_separator_agree() {
        for scheme in CeremonyDefinitionDigestScheme::KNOWN {
            assert_eq!(scheme.as_str().as_bytes(), scheme.domain_separator());
        }
    }
}
