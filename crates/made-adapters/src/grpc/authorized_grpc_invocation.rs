use made_core::value_objects::{AuthenticatedPrincipal, AuthorizationEvidence};

/// Authenticated principal and durable allow evidence for one gRPC invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedGrpcInvocation {
    principal: AuthenticatedPrincipal,
    evidence: AuthorizationEvidence,
}

impl AuthorizedGrpcInvocation {
    #[must_use]
    pub const fn new(principal: AuthenticatedPrincipal, evidence: AuthorizationEvidence) -> Self {
        Self {
            principal,
            evidence,
        }
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
    }

    #[must_use]
    pub const fn evidence(&self) -> &AuthorizationEvidence {
        &self.evidence
    }

    #[must_use]
    pub fn into_evidence(self) -> AuthorizationEvidence {
        self.evidence
    }
}
