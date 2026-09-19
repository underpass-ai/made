use super::{AuthenticatedPrincipal, AuthorizationEvidence};
use crate::DomainError;

/// Full authenticated identity and allow evidence for one admitted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedOperation {
    principal: AuthenticatedPrincipal,
    evidence: AuthorizationEvidence,
}

impl AuthorizedOperation {
    pub fn new(
        principal: AuthenticatedPrincipal,
        evidence: AuthorizationEvidence,
    ) -> Result<Self, DomainError> {
        if principal.id() != evidence.principal_id() {
            return Err(DomainError::InvariantViolated {
                reason: "authorization evidence principal must match the authenticated principal",
            });
        }
        Ok(Self {
            principal,
            evidence,
        })
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
    }

    #[must_use]
    pub const fn evidence(&self) -> &AuthorizationEvidence {
        &self.evidence
    }
}
