use std::sync::Arc;

use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionId,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    AuthorizedOperation, PrincipalKind,
};
use made_core::DomainError;

use super::{AuthorizationGateOutcome, AuthorizeOperationUseCase};

/// Authorization boundary for an explicitly configured in-process trusted host.
///
/// The caller supplies a transport invocation identity and the digest of the
/// canonical request. Actor fields from the business payload never select the
/// authenticated principal.
#[derive(Debug, Clone)]
pub struct TrustedHostAuthorizationGate {
    authorize: Arc<AuthorizeOperationUseCase>,
    principal: AuthenticatedPrincipal,
}

impl TrustedHostAuthorizationGate {
    pub fn new(
        authorize: Arc<AuthorizeOperationUseCase>,
        principal: AuthenticatedPrincipal,
    ) -> Result<Self, DomainError> {
        principal.validate()?;
        if principal.kind() != PrincipalKind::TrustedHost
            || principal.method() != AuthenticationMethod::LocalHostPolicy
        {
            return Err(DomainError::InvariantViolated {
                reason: "embedded authorization requires an explicit local trusted host",
            });
        }
        Ok(Self {
            authorize,
            principal,
        })
    }

    #[must_use]
    pub fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
    }

    pub async fn authorize(
        &self,
        request_id: AuthorizationRequestId,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        target_digest: AuthorizationTargetDigest,
        approval: Option<AuthorizationDecisionId>,
    ) -> Result<AuthorizedOperation, DomainError> {
        let mut request = AuthorizationRequest::new(
            request_id,
            self.principal.clone(),
            action,
            scope,
            target_digest,
        );
        if let Some(approval) = approval {
            request = request.with_approval(approval);
        }
        match self.authorize.execute(request).await? {
            AuthorizationGateOutcome::Allowed { evidence, .. } => {
                AuthorizedOperation::new(self.principal.clone(), evidence)
            }
            AuthorizationGateOutcome::Denied { .. } => Err(DomainError::InvariantViolated {
                reason: "authorization policy denied the embedded operation",
            }),
            AuthorizationGateOutcome::Expired { .. } => Err(DomainError::InvariantViolated {
                reason: "authorization decision expired before embedded dispatch",
            }),
        }
    }
}
