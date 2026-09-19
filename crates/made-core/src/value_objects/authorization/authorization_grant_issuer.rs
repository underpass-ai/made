use serde::{Deserialize, Serialize};

use super::{AuthorizationGrantId, PrincipalId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationGrantIssuer {
    principal_id: PrincipalId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_grant_id: Option<AuthorizationGrantId>,
}

impl AuthorizationGrantIssuer {
    #[must_use]
    pub const fn direct(principal_id: PrincipalId) -> Self {
        Self {
            principal_id,
            parent_grant_id: None,
        }
    }

    #[must_use]
    pub const fn delegated(
        principal_id: PrincipalId,
        parent_grant_id: AuthorizationGrantId,
    ) -> Self {
        Self {
            principal_id,
            parent_grant_id: Some(parent_grant_id),
        }
    }

    #[must_use]
    pub const fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    #[must_use]
    pub const fn parent_grant_id(&self) -> Option<&AuthorizationGrantId> {
        self.parent_grant_id.as_ref()
    }
}
