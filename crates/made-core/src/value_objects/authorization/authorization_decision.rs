use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    AuthorizationDecisionId, AuthorizationDecisionKind, AuthorizationDenialReason,
    AuthorizationEvidence, AuthorizationGrantId, AuthorizationPolicyVersion, AuthorizationRequest,
};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationDecision {
    id: AuthorizationDecisionId,
    request: AuthorizationRequest,
    policy_version: AuthorizationPolicyVersion,
    kind: AuthorizationDecisionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grant_id: Option<AuthorizationGrantId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    denial_reason: Option<AuthorizationDenialReason>,
    #[serde(with = "time::serde::rfc3339")]
    decided_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    valid_until: OffsetDateTime,
}

impl AuthorizationDecision {
    pub(crate) fn allow(
        request: AuthorizationRequest,
        policy_version: AuthorizationPolicyVersion,
        grant_id: Option<AuthorizationGrantId>,
        decided_at: OffsetDateTime,
        valid_until: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        Self::build(
            request,
            policy_version,
            AuthorizationDecisionKind::Allow,
            grant_id,
            None,
            decided_at,
            valid_until,
        )
    }

    pub(crate) fn deny(
        request: AuthorizationRequest,
        policy_version: AuthorizationPolicyVersion,
        denial_reason: AuthorizationDenialReason,
        decided_at: OffsetDateTime,
        valid_until: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        Self::build(
            request,
            policy_version,
            AuthorizationDecisionKind::Deny,
            None,
            Some(denial_reason),
            decided_at,
            valid_until,
        )
    }

    fn build(
        request: AuthorizationRequest,
        policy_version: AuthorizationPolicyVersion,
        kind: AuthorizationDecisionKind,
        grant_id: Option<AuthorizationGrantId>,
        denial_reason: Option<AuthorizationDenialReason>,
        decided_at: OffsetDateTime,
        valid_until: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let shape_is_valid = match kind {
            AuthorizationDecisionKind::Allow => denial_reason.is_none(),
            AuthorizationDecisionKind::Deny => grant_id.is_none() && denial_reason.is_some(),
        };
        if !shape_is_valid {
            return Err(DomainError::InvariantViolated {
                reason: "authorization decision fields contradict its outcome",
            });
        }
        if valid_until <= decided_at {
            return Err(DomainError::InvariantViolated {
                reason: "authorization decision expiry must follow admission",
            });
        }
        let canonical = serde_json::to_vec(&(
            &request,
            policy_version,
            kind,
            &grant_id,
            denial_reason,
            decided_at.unix_timestamp_nanos(),
            valid_until.unix_timestamp_nanos(),
        ))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "authorization decision cannot be canonicalized",
        })?;
        Ok(Self {
            id: AuthorizationDecisionId::for_bytes(&canonical),
            request,
            policy_version,
            kind,
            grant_id,
            denial_reason,
            decided_at,
            valid_until,
        })
    }

    #[must_use]
    pub fn id(&self) -> &AuthorizationDecisionId {
        &self.id
    }

    #[must_use]
    pub const fn request(&self) -> &AuthorizationRequest {
        &self.request
    }

    #[must_use]
    pub const fn policy_version(&self) -> AuthorizationPolicyVersion {
        self.policy_version
    }

    #[must_use]
    pub const fn kind(&self) -> AuthorizationDecisionKind {
        self.kind
    }

    #[must_use]
    pub fn grant_id(&self) -> Option<&AuthorizationGrantId> {
        self.grant_id.as_ref()
    }

    #[must_use]
    pub const fn denial_reason(&self) -> Option<AuthorizationDenialReason> {
        self.denial_reason
    }

    #[must_use]
    pub const fn decided_at(&self) -> OffsetDateTime {
        self.decided_at
    }

    #[must_use]
    pub const fn valid_until(&self) -> OffsetDateTime {
        self.valid_until
    }

    #[must_use]
    pub fn is_live_at(&self, now: OffsetDateTime) -> bool {
        now < self.valid_until
    }

    pub fn evidence(&self, now: OffsetDateTime) -> Result<AuthorizationEvidence, DomainError> {
        if self.kind != AuthorizationDecisionKind::Allow {
            return Err(DomainError::InvariantViolated {
                reason: "denied authorization decision has no admission evidence",
            });
        }
        if !self.is_live_at(now) {
            return Err(DomainError::InvariantViolated {
                reason: "authorization admission evidence has expired",
            });
        }
        Ok(AuthorizationEvidence::from_decision(self))
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        self.request.validate()?;
        let rebuilt = Self::build(
            self.request.clone(),
            self.policy_version,
            self.kind,
            self.grant_id.clone(),
            self.denial_reason,
            self.decided_at,
            self.valid_until,
        )?;
        if rebuilt.id != self.id {
            return Err(DomainError::InvariantViolated {
                reason: "authorization decision id does not seal its contents",
            });
        }
        Ok(())
    }
}
