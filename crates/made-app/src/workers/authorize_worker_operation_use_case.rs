use std::sync::Arc;

use super::{WorkerAuthorizationError, WorkerAuthorizationTarget};
use crate::authorization::{
    AcceptedStepCompletion, AuthorizationGateOutcome, ContinueAcceptedStepClaimUseCase,
    TrustedHostAuthorizationGate,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    AuthorizedOperation,
};
use made_core::DomainError;

/// Applies worker admission and continuation rules using the current authorization policy.
#[derive(Debug)]
pub struct AuthorizeWorkerOperationUseCase {
    gate: TrustedHostAuthorizationGate,
    continuation: Arc<ContinueAcceptedStepClaimUseCase>,
}

impl AuthorizeWorkerOperationUseCase {
    #[must_use]
    pub const fn new(
        gate: TrustedHostAuthorizationGate,
        continuation: Arc<ContinueAcceptedStepClaimUseCase>,
    ) -> Self {
        Self { gate, continuation }
    }

    pub async fn execute(
        &self,
        target: &WorkerAuthorizationTarget,
        request: AuthorizationRequestId,
        digest: AuthorizationTargetDigest,
    ) -> Result<AuthorizedOperation, WorkerAuthorizationError> {
        let scope = AuthorizationScope::Ceremony {
            ceremony_id: target.ceremony().clone(),
        };
        match target {
            WorkerAuthorizationTarget::EnforceDeadline { .. } => {
                self.authorize_direct(
                    request,
                    AuthorizationAction::EnforceCeremonyDeadlines,
                    scope,
                    digest,
                )
                .await
            }
            WorkerAuthorizationTarget::Claim { .. } => {
                self.authorize_direct(
                    request,
                    AuthorizationAction::ClaimCeremonyStep,
                    scope,
                    digest,
                )
                .await
            }
            WorkerAuthorizationTarget::Complete {
                ceremony,
                step,
                fence,
                ..
            } => {
                let input = AcceptedStepCompletion::from_invocation(
                    ceremony.clone(),
                    step.clone(),
                    fence.clone(),
                    self.gate.principal().clone(),
                    &request,
                    digest,
                )
                .map_err(WorkerAuthorizationError::Failure)?;
                self.continuation
                    .execute(input)
                    .await
                    .map_err(WorkerAuthorizationError::Failure)
            }
            WorkerAuthorizationTarget::Renew {
                ceremony,
                step,
                fence,
                ..
            } => {
                let input = AcceptedStepCompletion::from_invocation(
                    ceremony.clone(),
                    step.clone(),
                    fence.clone(),
                    self.gate.principal().clone(),
                    &request,
                    digest,
                )
                .map_err(WorkerAuthorizationError::Failure)?;
                self.continuation
                    .execute_renewal(input)
                    .await
                    .map_err(WorkerAuthorizationError::Failure)
            }
        }
    }

    async fn authorize_direct(
        &self,
        request: AuthorizationRequestId,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        digest: AuthorizationTargetDigest,
    ) -> Result<AuthorizedOperation, WorkerAuthorizationError> {
        match self
            .gate
            .authorize_outcome(request, action, scope, digest, None)
            .await
            .map_err(WorkerAuthorizationError::Failure)?
        {
            AuthorizationGateOutcome::Allowed { evidence, .. } => {
                AuthorizedOperation::new(self.gate.principal().clone(), evidence)
                    .map_err(WorkerAuthorizationError::Failure)
            }
            AuthorizationGateOutcome::Denied { .. } => Err(WorkerAuthorizationError::Denied(
                DomainError::InvariantViolated {
                    reason: "authorization policy denied worker admission",
                },
            )),
            AuthorizationGateOutcome::Expired { .. } => Err(WorkerAuthorizationError::Denied(
                DomainError::InvariantViolated {
                    reason: "authorization decision expired before worker admission",
                },
            )),
        }
    }
}
