use super::{CeremonyChangeImpact, CeremonyChangeKind, CeremonyValidationLocus};

/// One classified change between two ceremony definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDefinitionChange {
    kind: CeremonyChangeKind,
    locus: CeremonyValidationLocus,
    impact: CeremonyChangeImpact,
    detail: &'static str,
}

impl CeremonyDefinitionChange {
    pub(super) const fn new(
        kind: CeremonyChangeKind,
        locus: CeremonyValidationLocus,
        impact: CeremonyChangeImpact,
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
    pub const fn kind(&self) -> CeremonyChangeKind {
        self.kind
    }

    #[must_use]
    pub const fn locus(&self) -> &CeremonyValidationLocus {
        &self.locus
    }

    #[must_use]
    pub const fn impact(&self) -> CeremonyChangeImpact {
        self.impact
    }

    #[must_use]
    pub const fn detail(&self) -> &'static str {
        self.detail
    }
}
