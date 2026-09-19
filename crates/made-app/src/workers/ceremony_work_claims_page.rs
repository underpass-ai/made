use made_core::value_objects::CeremonyId;

use super::{CeremonyWorkClaimFailure, ExecuteCeremonyOperationInput};

/// Claims and per-ceremony failures from one keyset discovery page.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyWorkClaimsPage {
    claims: Vec<ExecuteCeremonyOperationInput>,
    failures: Vec<CeremonyWorkClaimFailure>,
    next_cursor: Option<CeremonyId>,
}

impl CeremonyWorkClaimsPage {
    #[must_use]
    pub const fn new(
        claims: Vec<ExecuteCeremonyOperationInput>,
        failures: Vec<CeremonyWorkClaimFailure>,
        next_cursor: Option<CeremonyId>,
    ) -> Self {
        Self {
            claims,
            failures,
            next_cursor,
        }
    }

    #[must_use]
    pub fn claims(&self) -> &[ExecuteCeremonyOperationInput] {
        &self.claims
    }

    #[must_use]
    pub fn failures(&self) -> &[CeremonyWorkClaimFailure] {
        &self.failures
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&CeremonyId> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<ExecuteCeremonyOperationInput>,
        Vec<CeremonyWorkClaimFailure>,
        Option<CeremonyId>,
    ) {
        (self.claims, self.failures, self.next_cursor)
    }
}
