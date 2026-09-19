use std::sync::Arc;

use made_app::authorization::{
    AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
    ContinueAcceptedCeremonyWorkUseCase, ReadAuthorizationDecisionsUseCase,
    ReadAuthorizationPolicyUseCase,
};
use made_core::ports::{AuthorizationDecisionPage, AuthorizationPolicySnapshot};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationDecisionId, AuthorizationDecisionPageLimit,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationRevocationReason,
};
use made_core::DomainError;

#[derive(Clone, Debug)]
pub(crate) struct EmbeddedAuthorizationServices {
    policy: Arc<ReadAuthorizationPolicyUseCase>,
    decisions: Arc<ReadAuthorizationDecisionsUseCase>,
    administration: Arc<AuthorizationPolicyAdministrationService>,
    continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

impl EmbeddedAuthorizationServices {
    pub(crate) fn new(
        policy: ReadAuthorizationPolicyUseCase,
        decisions: ReadAuthorizationDecisionsUseCase,
        administration: AuthorizationPolicyAdministrationService,
        continuation: ContinueAcceptedCeremonyWorkUseCase,
    ) -> Self {
        Self {
            policy: Arc::new(policy),
            decisions: Arc::new(decisions),
            administration: Arc::new(administration),
            continuation: Arc::new(continuation),
        }
    }

    pub(crate) fn continuation(&self) -> Arc<ContinueAcceptedCeremonyWorkUseCase> {
        self.continuation.clone()
    }

    pub(crate) async fn policy(&self) -> Result<AuthorizationPolicySnapshot, DomainError> {
        active_operation(AuthorizationAction::ReadAuthorizationPolicy)?;
        self.policy.execute().await
    }

    pub(crate) async fn decisions(
        &self,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        active_operation(AuthorizationAction::ReadAuthorizationDecisions)?;
        self.decisions.execute(after, limit).await
    }

    pub(crate) async fn issue(
        &self,
        grant: AuthorizationGrant,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        let operation = active_operation(AuthorizationAction::IssueAuthorizationGrant)?;
        if grant.issued_by() != operation.principal() {
            return Err(DomainError::InvariantViolated {
                reason: "authorization grant issuer must be the authenticated facade principal",
            });
        }
        self.administration
            .issue(operation.principal(), grant)
            .await
    }

    pub(crate) async fn revoke(
        &self,
        grant_id: &AuthorizationGrantId,
        reason: AuthorizationRevocationReason,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        let operation = active_operation(AuthorizationAction::RevokeAuthorizationGrant)?;
        self.administration
            .revoke(operation.principal(), grant_id, reason)
            .await
    }
}

fn active_operation(
    expected: AuthorizationAction,
) -> Result<made_core::value_objects::AuthorizedOperation, DomainError> {
    let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
        DomainError::InvariantViolated {
            reason: "protected embedded facade requires an authorized operation context",
        },
    )?;
    if operation.evidence().action() != expected {
        return Err(DomainError::InvariantViolated {
            reason: "authorized operation action does not match the embedded facade method",
        });
    }
    Ok(operation)
}
