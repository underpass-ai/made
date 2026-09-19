use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    AuthorizationAction, AuthorizationDecision, AuthorizationDecisionId,
    AuthorizationPolicyVersion, AuthorizationRequestId, AuthorizationScope,
    AuthorizationTargetDigest, PrincipalId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationEvidence {
    decision_id: AuthorizationDecisionId,
    request_id: AuthorizationRequestId,
    principal_id: PrincipalId,
    action: AuthorizationAction,
    scope: AuthorizationScope,
    target_digest: AuthorizationTargetDigest,
    policy_version: AuthorizationPolicyVersion,
    #[serde(with = "time::serde::rfc3339")]
    admitted_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    valid_until: OffsetDateTime,
}

impl AuthorizationEvidence {
    pub(crate) fn from_decision(decision: &AuthorizationDecision) -> Self {
        let request = decision.request();
        Self {
            decision_id: decision.id().clone(),
            request_id: request.id().clone(),
            principal_id: request.principal().id().clone(),
            action: request.action(),
            scope: request.scope().clone(),
            target_digest: request.target_digest().clone(),
            policy_version: decision.policy_version(),
            admitted_at: decision.decided_at(),
            valid_until: decision.valid_until(),
        }
    }

    #[must_use]
    pub fn decision_id(&self) -> &AuthorizationDecisionId {
        &self.decision_id
    }

    #[must_use]
    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    #[must_use]
    pub const fn action(&self) -> AuthorizationAction {
        self.action
    }

    #[must_use]
    pub const fn scope(&self) -> &AuthorizationScope {
        &self.scope
    }

    #[must_use]
    pub fn target_digest(&self) -> &AuthorizationTargetDigest {
        &self.target_digest
    }

    #[must_use]
    pub const fn policy_version(&self) -> AuthorizationPolicyVersion {
        self.policy_version
    }

    #[must_use]
    pub const fn admitted_at(&self) -> OffsetDateTime {
        self.admitted_at
    }

    #[must_use]
    pub const fn valid_until(&self) -> OffsetDateTime {
        self.valid_until
    }

    #[must_use]
    pub fn is_live_at(&self, now: OffsetDateTime) -> bool {
        now < self.valid_until
    }
}
