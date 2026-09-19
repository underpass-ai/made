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
        if local_policy && !trusted_host {
            return Err(DomainError::InvariantViolated {
                reason: "local host policy authenticates only trusted-host principals",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_host_may_be_authenticated_by_mutual_tls() {
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new("remote-owner").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::MutualTls,
        );

        assert!(principal.is_ok());
    }

    #[test]
    fn local_host_policy_rejects_non_host_principals() {
        for kind in [PrincipalKind::Human, PrincipalKind::Worker] {
            let result = AuthenticatedPrincipal::new(
                PrincipalId::new("not-a-host").unwrap(),
                kind,
                AuthenticationMethod::LocalHostPolicy,
            );

            assert!(matches!(result, Err(DomainError::InvariantViolated { .. })));
        }
    }
}
