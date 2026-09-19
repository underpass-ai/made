use serde::{Deserialize, Serialize};

use super::{AuthenticationMethod, PrincipalId, PrincipalKind};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedPrincipal {
    id: PrincipalId,
    kind: PrincipalKind,
    method: AuthenticationMethod,
}

impl AuthenticatedPrincipal {
    pub fn new(
        id: PrincipalId,
        kind: PrincipalKind,
        method: AuthenticationMethod,
    ) -> Result<Self, DomainError> {
        let principal = Self { id, kind, method };
        principal.validate()?;
        Ok(principal)
    }

    #[must_use]
    pub fn id(&self) -> &PrincipalId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> PrincipalKind {
        self.kind
    }

    #[must_use]
    pub const fn method(&self) -> AuthenticationMethod {
        self.method
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        let trusted_host = self.kind == PrincipalKind::TrustedHost;
        let local_policy = self.method == AuthenticationMethod::LocalHostPolicy;
        if trusted_host != local_policy {
            return Err(DomainError::InvariantViolated {
                reason: "local host policy authenticates exactly trusted-host principals",
            });
        }
        Ok(())
    }
}
