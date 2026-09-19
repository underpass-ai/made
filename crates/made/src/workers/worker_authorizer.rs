use async_trait::async_trait;
use made_app::authorization::AuthorizationGateOutcome;
use made_app::authorization::{
    AcceptedStepCompletion, ContinueAcceptedStepClaimUseCase, TrustedHostAuthorizationGate,
};
use made_app::workers::{
    WorkerAuthorizationError, WorkerAuthorizationPort, WorkerAuthorizationTarget,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    AuthorizedOperation,
};
use made_core::DomainError;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct WorkerAuthorizer {
    gate: TrustedHostAuthorizationGate,
    continuation: Arc<ContinueAcceptedStepClaimUseCase>,
}

impl WorkerAuthorizer {
    pub(crate) const fn new(
        gate: TrustedHostAuthorizationGate,
        continuation: Arc<ContinueAcceptedStepClaimUseCase>,
    ) -> Self {
        Self { gate, continuation }
    }

    fn target(
        target: &WorkerAuthorizationTarget,
    ) -> Result<(AuthorizationRequestId, AuthorizationTargetDigest), DomainError> {
        let bytes = match target {
            WorkerAuthorizationTarget::EnforceDeadline { ceremony } => {
                serde_json::to_vec(&("worker-deadline-v1", ceremony))
            }
            WorkerAuthorizationTarget::Claim {
                ceremony,
                step,
                operation,
                owner,
                lease_ttl,
            } => serde_json::to_vec(&(
                "worker-claim-v1",
                ceremony,
                step,
                operation,
                owner,
                lease_ttl.get(),
            )),
            WorkerAuthorizationTarget::Complete {
                ceremony,
                step,
                operation,
                fence,
            } => serde_json::to_vec(&("worker-completion-v1", ceremony, step, operation, fence)),
            WorkerAuthorizationTarget::Renew {
                ceremony,
                step,
                operation,
                fence,
            } => serde_json::to_vec(&("worker-renewal-v1", ceremony, step, operation, fence)),
        }
        .map_err(|_| DomainError::InvariantViolated {
            reason: "worker authorization target cannot be canonicalized",
        })?;
        let digest = AuthorizationTargetDigest::for_bytes(&bytes);
        let request = AuthorizationRequestId::new(format!("worker:{}", uuid::Uuid::new_v4()))?;
        Ok((request, digest))
    }
}

#[async_trait]
impl WorkerAuthorizationPort for WorkerAuthorizer {
    async fn authorize(
        &self,
        target: &WorkerAuthorizationTarget,
    ) -> Result<AuthorizedOperation, WorkerAuthorizationError> {
        let (request, digest) = Self::target(target).map_err(WorkerAuthorizationError::Failure)?;
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
}

impl WorkerAuthorizer {
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use made_adapters::memory::{InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore};
    use made_app::authorization::{
        AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
        ContinueAcceptedCeremonyWorkUseCase, ContinueAcceptedStepClaimUseCase,
        TrustedHostAuthorizationGate,
    };
    use made_app::services::SessionStream;
    use made_core::ports::{ClockPort, NoopCeremonyEventSubscriber};
    use made_core::value_objects::{
        AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
        AuthorizationDecisionTtl, AuthorizationGrant, AuthorizationGrantId,
        AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope, DelegationDepth,
        PrincipalId, PrincipalKind,
    };
    use made_core::value_objects::{CeremonyId, ExecutionOperationId, StepClaimFence, StepId};
    use time::{macros::datetime, Duration, OffsetDateTime};

    use super::*;

    #[test]
    fn renewal_keeps_the_claim_source_stable_but_uses_a_fresh_request_identity() {
        let target = WorkerAuthorizationTarget::Renew {
            ceremony: CeremonyId::new("poll-target").unwrap(),
            step: StepId::new("work").unwrap(),
            operation: ExecutionOperationId::new("1".repeat(64)).unwrap(),
            fence: StepClaimFence::new("2".repeat(64)).unwrap(),
        };

        let first = WorkerAuthorizer::target(&target).unwrap();
        let second = WorkerAuthorizer::target(&target).unwrap();

        assert_ne!(first.0, second.0);
        assert_eq!(first.1, second.1);
    }

    #[derive(Debug)]
    struct MutableClock(RwLock<OffsetDateTime>);

    impl ClockPort for MutableClock {
        fn now(&self) -> OffsetDateTime {
            *self.0.read().unwrap()
        }
    }

    #[tokio::test]
    async fn repeated_poll_after_decision_ttl_is_reauthorized() {
        let now = datetime!(2026-09-19 12:00:00 UTC);
        let clock = Arc::new(MutableClock(RwLock::new(now)));
        let store = Arc::new(InMemoryAuthorizationPolicyStore::new());
        let policy_id = AuthorizationPolicyId::new("worker-poll-policy").unwrap();
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new("worker-host").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        let administration = AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            store.clone(),
            clock.clone(),
        );
        administration
            .open(principal.clone(), Vec::new())
            .await
            .unwrap();
        let ceremony = CeremonyId::new("poll-target").unwrap();
        administration
            .issue(
                &principal,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new("worker-poll-grant").unwrap(),
                    principal.id().clone(),
                    [AuthorizationAction::EnforceCeremonyDeadlines],
                    AuthorizationScope::Ceremony {
                        ceremony_id: ceremony.clone(),
                    },
                    (now, None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(principal.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            policy_id.clone(),
            store.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(1).unwrap(),
        ));
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        let stream = Arc::new(SessionStream::new(
            events.clone(),
            events,
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let continuation = Arc::new(
            ContinueAcceptedStepClaimUseCase::new(
                stream,
                Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
                    policy_id,
                    store,
                    clock.clone(),
                    AuthorizationDecisionTtl::from_seconds(1).unwrap(),
                )),
                clock.clone(),
            )
            .with_reauthorization(authorize.clone()),
        );
        let authorizer = WorkerAuthorizer::new(
            TrustedHostAuthorizationGate::new(authorize, principal).unwrap(),
            continuation,
        );
        let target = WorkerAuthorizationTarget::EnforceDeadline { ceremony };

        authorizer.authorize(&target).await.unwrap();
        *clock.0.write().unwrap() = now + Duration::seconds(2);
        authorizer.authorize(&target).await.unwrap();
    }
}
