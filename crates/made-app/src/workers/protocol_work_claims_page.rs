use made_core::value_objects::ProtocolId;

use super::{ExecuteProtocolOperationInput, ProtocolWorkClaimFailure};

/// Claims and per-protocol failures from one keyset discovery page.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolWorkClaimsPage {
    claims: Vec<ExecuteProtocolOperationInput>,
    failures: Vec<ProtocolWorkClaimFailure>,
    next_cursor: Option<ProtocolId>,
}

impl ProtocolWorkClaimsPage {
    #[must_use]
    pub const fn new(
        claims: Vec<ExecuteProtocolOperationInput>,
        failures: Vec<ProtocolWorkClaimFailure>,
        next_cursor: Option<ProtocolId>,
    ) -> Self {
        Self {
            claims,
            failures,
            next_cursor,
        }
    }

    #[must_use]
    pub fn claims(&self) -> &[ExecuteProtocolOperationInput] {
        &self.claims
    }

    #[must_use]
    pub fn failures(&self) -> &[ProtocolWorkClaimFailure] {
        &self.failures
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ProtocolId> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<ExecuteProtocolOperationInput>,
        Vec<ProtocolWorkClaimFailure>,
        Option<ProtocolId>,
    ) {
        (self.claims, self.failures, self.next_cursor)
    }
}
