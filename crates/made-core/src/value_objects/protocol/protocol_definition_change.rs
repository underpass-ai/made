use super::{ProtocolChangeImpact, ProtocolChangeKind, ProtocolValidationLocus};

/// One classified change between two protocol definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDefinitionChange {
    kind: ProtocolChangeKind,
    locus: ProtocolValidationLocus,
    impact: ProtocolChangeImpact,
    detail: &'static str,
}

impl ProtocolDefinitionChange {
    pub(super) const fn new(
        kind: ProtocolChangeKind,
        locus: ProtocolValidationLocus,
        impact: ProtocolChangeImpact,
        detail: &'static str,
    ) -> Self {
        Self {
            kind,
            locus,
            impact,
            detail,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ProtocolChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn locus(&self) -> &ProtocolValidationLocus {
        &self.locus
    }

    #[must_use]
    pub const fn impact(&self) -> ProtocolChangeImpact {
        self.impact
    }

    #[must_use]
    pub const fn detail(&self) -> &'static str {
        self.detail
    }
}
