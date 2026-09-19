use made_app::authorization::AuthorizationMutationOutcome;
use made_core::ports::{AuthorizationDecisionPage, AuthorizationPolicySnapshot};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationDecision, AuthorizationDecisionId,
    AuthorizationDecisionPageLimit, AuthorizationGrant, AuthorizationGrantId,
    AuthorizationRequestId, AuthorizationRevocationReason, AuthorizationScope,
    AuthorizationTargetDigest,
};
use made_core::DomainError;

use super::EmbeddedMade;
use crate::embedded_authorization_services::EmbeddedAuthorizationServices;

impl EmbeddedMade {
    pub async fn authorization_policy(&self) -> Result<AuthorizationPolicySnapshot, DomainError> {
        self.authorization()?.policy().await
    }

    pub async fn authorization_decisions(
        &self,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        self.authorization()?.decisions(after, limit).await
    }

    pub async fn issue_authorization_grant(
        &self,
        grant: AuthorizationGrant,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        self.authorization()?.issue(grant).await
    }

    pub async fn revoke_authorization_grant(
        &self,
        grant_id: &AuthorizationGrantId,
        reason: AuthorizationRevocationReason,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        self.authorization()?.revoke(grant_id, reason).await
    }

    /// Admit the approval half of a configured separation rule.
    ///
    /// A direct Rust caller passes the returned decision id to
    /// [`made_app::authorization::TrustedHostAuthorizationGate::authorize`]
    /// for the exact execution
    /// request, then runs the facade future inside
    /// [`made_app::services::AuthorizationOperationScope`]. The facade checks
    /// that typed context again for the method's action and resource scope.
    pub async fn approve_authorization_operation(
        &self,
        request_id: AuthorizationRequestId,
        approval_action: AuthorizationAction,
        execution_action: AuthorizationAction,
        scope: AuthorizationScope,
        target_digest: AuthorizationTargetDigest,
    ) -> Result<AuthorizationDecision, DomainError> {
        self.ceremony_search_authorization
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "embedded operation approval requires an explicit authorization gate",
            })?
            .approve_operation(
                request_id,
                approval_action,
                execution_action,
                scope,
                target_digest,
            )
            .await
    }

    fn authorization(&self) -> Result<&EmbeddedAuthorizationServices, DomainError> {
        self.authorization
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "embedded authorization policy services are not configured",
            })
    }
}
