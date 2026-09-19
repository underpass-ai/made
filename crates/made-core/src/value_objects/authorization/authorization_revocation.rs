use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{AuthorizationGrantId, AuthorizationRevocationReason, PrincipalId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationRevocation {
    grant_id: AuthorizationGrantId,
    revoked_by: PrincipalId,
    reason: AuthorizationRevocationReason,
    #[serde(with = "time::serde::rfc3339")]
    revoked_at: OffsetDateTime,
}

impl AuthorizationRevocation {
    #[must_use]
    pub const fn new(
        grant_id: AuthorizationGrantId,
        revoked_by: PrincipalId,
        reason: AuthorizationRevocationReason,
        revoked_at: OffsetDateTime,
    ) -> Self {
        Self {
            grant_id,
            revoked_by,
            reason,
            revoked_at,
        }
    }

    #[must_use]
    pub const fn grant_id(&self) -> &AuthorizationGrantId {
        &self.grant_id
    }

    #[must_use]
    pub const fn revoked_by(&self) -> &PrincipalId {
        &self.revoked_by
    }

    #[must_use]
    pub const fn reason(&self) -> &AuthorizationRevocationReason {
        &self.reason
    }

    #[must_use]
    pub const fn revoked_at(&self) -> OffsetDateTime {
        self.revoked_at
    }
}
