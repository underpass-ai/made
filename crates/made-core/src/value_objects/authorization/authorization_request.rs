use serde::{Deserialize, Serialize};

use super::{
    AuthenticatedPrincipal, AuthorizationAction, AuthorizationDecisionId, AuthorizationRequestId,
    AuthorizationScope, AuthorizationTargetDigest,
};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    id: AuthorizationRequestId,
    principal: AuthenticatedPrincipal,
    action: AuthorizationAction,
    scope: AuthorizationScope,
    target_digest: AuthorizationTargetDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    approval_decision_id: Option<AuthorizationDecisionId>,
}

impl AuthorizationRequest {
    #[must_use]
    pub const fn new(
        id: AuthorizationRequestId,
        principal: AuthenticatedPrincipal,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        target_digest: AuthorizationTargetDigest,
    ) -> Self {
        Self {
            id,
            principal,
            action,
            scope,
            target_digest,
            approval_decision_id: None,
        }
    }

    #[must_use]
    pub fn with_approval(mut self, approval_decision_id: AuthorizationDecisionId) -> Self {
        self.approval_decision_id = Some(approval_decision_id);
        self
    }
    #[must_use]
    pub fn id(&self) -> &AuthorizationRequestId {
        &self.id
    }
    #[must_use]
    pub fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
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
    pub fn approval_decision_id(&self) -> Option<&AuthorizationDecisionId> {
        self.approval_decision_id.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        self.principal.validate()
    }
}
