use std::sync::Arc;

use async_trait::async_trait;
use made_app::workers::{
    AuthorizeWorkerOperationUseCase, WorkerAuthorizationError, WorkerAuthorizationPort,
    WorkerAuthorizationTarget,
};
use made_core::value_objects::{
    AuthorizationRequestId, AuthorizationTargetDigest, AuthorizedOperation,
};
use made_core::DomainError;

/// Encodes a worker invocation and supplies a fresh authorization request identity.
#[derive(Debug)]
pub struct WorkerAuthorizer {
    authorize: Arc<AuthorizeWorkerOperationUseCase>,
}

impl WorkerAuthorizer {
    #[must_use]
    pub const fn new(authorize: Arc<AuthorizeWorkerOperationUseCase>) -> Self {
        Self { authorize }
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
        self.authorize.execute(target, request, digest).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, RwLock};

    use crate::memory::{InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore};
    use made_app::authorization::{
        AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
        ContinueAcceptedCeremonyWorkUseCase, ContinueAcceptedStepClaimUseCase,
        TrustedHostAuthorizationGate,
    };
    use made_app::services::SessionStream;
    use made_core::entities::ceremony_events::{CeremonyInstanceStarted, StepStarted};
    use made_core::entities::{AuditFact, CeremonyEvent};
    use made_core::ports::{
        AuthorizationPolicyStorePort, CeremonyEventStorePort, ClockPort,
        NoopCeremonyEventSubscriber,
    };
    use made_core::value_objects::{
        AuditActor, AuditActorKind, AuthenticatedPrincipal, AuthenticationMethod,
        AuthorizationAction, AuthorizationDecisionTtl, AuthorizationGrant, AuthorizationGrantId,
        AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope, CeremonyContext,
        CeremonyName, CeremonyVersion, DelegationDepth, DurationMs, EventId, IdempotencyKey,
        LeaseOwnerId, PrincipalId, PrincipalKind, RoleId, StateId, StateIteration, StateVisit,
        StepAttempt, StepIteration, StepLease, StreamVersion,
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
        let authorizer = WorkerAuthorizer::new(Arc::new(AuthorizeWorkerOperationUseCase::new(
            TrustedHostAuthorizationGate::new(authorize, principal).unwrap(),
            continuation,
        )));
        let target = WorkerAuthorizationTarget::EnforceDeadline { ceremony };

        authorizer.authorize(&target).await.unwrap();
        *clock.0.write().unwrap() = now + Duration::seconds(2);
        authorizer.authorize(&target).await.unwrap();
    }

    #[tokio::test]
    async fn renewal_crossing_claim_decision_ttl_revalidates_the_sealed_source() {
        let now = datetime!(2026-09-19 12:00:00 UTC);
        let (authorizer, target, source_decision, clock, policies, policy_id) =
            renewal_fixture(now).await;

        *clock.0.write().unwrap() = now + Duration::seconds(2);
        let renewed = authorizer.authorize(&target).await.unwrap();

        assert_eq!(
            renewed.evidence().action(),
            AuthorizationAction::RenewCeremonyStepLease
        );
        let decision = policies
            .decision(&policy_id, renewed.evidence().decision_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            decision.request().accepted_work_decision_id(),
            Some(&source_decision)
        );
    }

    async fn renewal_fixture(
        now: OffsetDateTime,
    ) -> (
        WorkerAuthorizer,
        WorkerAuthorizationTarget,
        made_core::value_objects::AuthorizationDecisionId,
        Arc<MutableClock>,
        Arc<InMemoryAuthorizationPolicyStore>,
        AuthorizationPolicyId,
    ) {
        let (clock, policies, policy_id, authorize, gate, ceremony) = renewal_policy(now).await;
        let claim = gate
            .authorize(
                AuthorizationRequestId::new("renew-source-claim").unwrap(),
                AuthorizationAction::ClaimCeremonyStep,
                AuthorizationScope::Ceremony {
                    ceremony_id: ceremony.clone(),
                },
                AuthorizationTargetDigest::for_bytes(b"renew-source-claim"),
                None,
            )
            .await
            .unwrap();
        let step = StepId::new("work").unwrap();
        let operation = ExecutionOperationId::new("3".repeat(64)).unwrap();
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        let fence = append_renewal_source(
            events.clone(),
            &ceremony,
            &step,
            now,
            claim.evidence().clone(),
        )
        .await;
        let stream = Arc::new(SessionStream::new(
            events.clone(),
            events,
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let continuation = Arc::new(
            ContinueAcceptedStepClaimUseCase::new(
                stream,
                Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
                    policy_id.clone(),
                    policies.clone(),
                    clock.clone(),
                    AuthorizationDecisionTtl::from_seconds(1).unwrap(),
                )),
                clock.clone(),
            )
            .with_reauthorization(authorize),
        );
        let target = WorkerAuthorizationTarget::Renew {
            ceremony,
            step,
            operation,
            fence,
        };
        (
            WorkerAuthorizer::new(Arc::new(AuthorizeWorkerOperationUseCase::new(
                gate,
                continuation,
            ))),
            target,
            claim.evidence().decision_id().clone(),
            clock,
            policies,
            policy_id,
        )
    }

    async fn renewal_policy(
        now: OffsetDateTime,
    ) -> (
        Arc<MutableClock>,
        Arc<InMemoryAuthorizationPolicyStore>,
        AuthorizationPolicyId,
        Arc<AuthorizeOperationUseCase>,
        TrustedHostAuthorizationGate,
        CeremonyId,
    ) {
        let clock = Arc::new(MutableClock(RwLock::new(now)));
        let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
        let policy_id = AuthorizationPolicyId::new("worker-renew-policy").unwrap();
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new("worker-host").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        let administration = AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            policies.clone(),
            clock.clone(),
        );
        administration
            .open(principal.clone(), Vec::new())
            .await
            .unwrap();
        let ceremony = CeremonyId::new("renew-target").unwrap();
        administration
            .issue(
                &principal,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new("worker-renew-grant").unwrap(),
                    principal.id().clone(),
                    [AuthorizationAction::ClaimCeremonyStep],
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
            policies.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(1).unwrap(),
        ));
        let gate = TrustedHostAuthorizationGate::new(authorize.clone(), principal).unwrap();
        (clock, policies, policy_id, authorize, gate, ceremony)
    }

    async fn append_renewal_source(
        events: Arc<InMemoryCeremonyEventStore>,
        ceremony: &CeremonyId,
        step: &StepId,
        now: OffsetDateTime,
        evidence: made_core::value_objects::AuthorizationEvidence,
    ) -> StepClaimFence {
        let lease = StepLease::acquire(
            LeaseOwnerId::new("worker-owner").unwrap(),
            IdempotencyKey::new("renew-source").unwrap(),
            now,
            DurationMs::from_millis(30_000),
        )
        .unwrap();
        let claimed = StepStarted {
            step_id: step.clone(),
            state_visit: Some(StateVisit::FIRST),
            state_iteration: Some(StateIteration::FIRST),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease,
            started_by: RoleId::new("WORKER").unwrap(),
            role_from: None,
            sealed_role: None,
            deadline: None,
            budget_reservation_id: None,
            started_at: now,
        };
        let fence = claimed.claim_fence(ceremony).unwrap();
        let definitions = CeremonyName::new("renew_test").unwrap();
        let version = CeremonyVersion::v1();
        let actor = AuditActor::new("worker", AuditActorKind::Service, None).unwrap();
        let fact = |event_id: &str, event| AuditFact {
            event_id: EventId::new(event_id).unwrap(),
            event,
            ceremony_id: ceremony.clone(),
            definition_name: definitions.clone(),
            definition_version: version.clone(),
            occurred_at: now,
            actor: actor.clone(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        };
        events
            .append_authorized(
                ceremony,
                StreamVersion::EMPTY,
                vec![
                    fact(
                        "renew-instance-started",
                        CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
                            ceremony_id: ceremony.clone(),
                            definition_name: definitions.clone(),
                            definition_version: version.clone(),
                            initial_state: StateId::new("OPEN").unwrap(),
                            step_ids: BTreeSet::from([step.clone()]),
                            context: CeremonyContext::empty(),
                            bound_definition: None,
                            lineage: None,
                            budget_account_id: None,
                            ceremony_deadline: None,
                            state_deadline: None,
                            created_at: now,
                        }),
                    ),
                    fact("renew-step-started", CeremonyEvent::StepStarted(claimed)),
                ],
                evidence,
            )
            .await
            .unwrap();
        fence
    }
}
