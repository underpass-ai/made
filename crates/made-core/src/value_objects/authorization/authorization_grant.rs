use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    AuthorizationAction, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationScope,
    DelegationDepth, PrincipalId,
};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationGrant {
    id: AuthorizationGrantId,
    grantee: PrincipalId,
    actions: BTreeSet<AuthorizationAction>,
    scope: AuthorizationScope,
    #[serde(with = "time::serde::rfc3339")]
    valid_from: OffsetDateTime,
    #[serde(default, with = "time::serde::rfc3339::option")]
    valid_until: Option<OffsetDateTime>,
    delegation_depth: DelegationDepth,
    issuer: AuthorizationGrantIssuer,
}

impl AuthorizationGrant {
    pub fn new(
        id: AuthorizationGrantId,
        grantee: PrincipalId,
        actions: impl IntoIterator<Item = AuthorizationAction>,
        scope: AuthorizationScope,
        validity: (OffsetDateTime, Option<OffsetDateTime>),
        delegation_depth: DelegationDepth,
        issuer: AuthorizationGrantIssuer,
    ) -> Result<Self, DomainError> {
        let grant = Self {
            id,
            grantee,
            actions: actions.into_iter().collect(),
            scope,
            valid_from: validity.0,
            valid_until: validity.1,
            delegation_depth,
            issuer,
        };
        grant.validate()?;
        Ok(grant)
    }

    #[must_use]
    pub fn id(&self) -> &AuthorizationGrantId {
        &self.id
    }
    #[must_use]
    pub fn grantee(&self) -> &PrincipalId {
        &self.grantee
    }
    #[must_use]
    pub fn actions(&self) -> &BTreeSet<AuthorizationAction> {
        &self.actions
    }
    #[must_use]
    pub const fn scope(&self) -> &AuthorizationScope {
        &self.scope
    }
    #[must_use]
    pub const fn valid_from(&self) -> OffsetDateTime {
        self.valid_from
    }
    #[must_use]
    pub const fn valid_until(&self) -> Option<OffsetDateTime> {
        self.valid_until
    }
    #[must_use]
    pub const fn delegation_depth(&self) -> DelegationDepth {
        self.delegation_depth
    }
    #[must_use]
    pub const fn issued_by(&self) -> &crate::value_objects::AuthenticatedPrincipal {
        self.issuer.principal()
    }
    #[must_use]
    pub const fn parent_grant_id(&self) -> Option<&AuthorizationGrantId> {
        self.issuer.parent_grant_id()
    }
    #[must_use]
    pub fn is_active_at(&self, now: OffsetDateTime) -> bool {
        self.valid_from <= now && self.valid_until.is_none_or(|until| now < until)
    }
    #[must_use]
    pub fn permits(
        &self,
        action: AuthorizationAction,
        scope: &AuthorizationScope,
        now: OffsetDateTime,
    ) -> bool {
        self.is_active_at(now) && self.actions.contains(&action) && self.scope.covers(scope)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        self.issuer.principal().validate()?;
        if self.scope.is_resolved_request_scope() {
            return Err(DomainError::InvariantViolated {
                reason:
                    "resolved ceremony lineage is an authorization request scope, not a grant scope",
            });
        }
        if self.actions.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "authorization_grant.actions",
            });
        }
        if self
            .valid_until
            .is_some_and(|until| until <= self.valid_from)
        {
            return Err(DomainError::InvariantViolated {
                reason: "authorization grant expiry must follow its validity start",
            });
        }
        Ok(())
    }
}
