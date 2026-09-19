use serde::{Deserialize, Serialize};

use super::{AuthenticatedPrincipal, AuthorizationGrantId, PrincipalId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationGrantIssuer {
    principal: AuthenticatedPrincipal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_grant_id: Option<AuthorizationGrantId>,
}

impl AuthorizationGrantIssuer {
    #[must_use]
    pub const fn direct(principal: AuthenticatedPrincipal) -> Self {
        Self {
            principal,
            parent_grant_id: None,
        }
    }

    #[must_use]
    pub const fn delegated(
        principal: AuthenticatedPrincipal,
        parent_grant_id: AuthorizationGrantId,
    ) -> Self {
        Self {
            principal,
            parent_grant_id: Some(parent_grant_id),
        }
    }

    #[must_use]
    pub const fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
    }

    #[must_use]
    pub fn principal_id(&self) -> &PrincipalId {
        self.principal.id()
    }

    #[must_use]
    pub const fn parent_grant_id(&self) -> Option<&AuthorizationGrantId> {
        self.parent_grant_id.as_ref()
    }
}
