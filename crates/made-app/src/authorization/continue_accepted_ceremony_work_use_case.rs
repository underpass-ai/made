use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::ports::{AuthorizationPolicyAppendOutcome, AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationDecisionTtl, AuthorizationPolicyId, AuthorizationRequest,
    AuthorizationRequestId, AuthorizationTargetDigest, AuthorizedOperation,
};
use made_core::DomainError;

const MAX_CONFLICT_RETRIES: usize = 16;

#[derive(Clone)]
pub struct ContinueAcceptedCeremonyWorkUseCase {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<dyn ClockPort>,
    ttl: AuthorizationDecisionTtl,
}

impl ContinueAcceptedCeremonyWorkUseCase {
    #[must_use]
    pub fn new(
        policy_id: AuthorizationPolicyId,
        store: Arc<dyn AuthorizationPolicyStorePort>,
        clock: Arc<dyn ClockPort>,
        ttl: AuthorizationDecisionTtl,
    ) -> Self {
        Self {
            policy_id,
            store,
            clock,
            ttl,
        }
    }

    /// Mint current evidence for a recovery effect already admitted and
    /// sealed by `source`. The source event, not a broker payload, supplies
    /// the antecedent decision and authenticated principal.
    pub async fn execute(
        &self,
        source: &AuditRecord,
        request_id: AuthorizationRequestId,
    ) -> Result<AuthorizedOperation, DomainError> {
        let source_evidence =
            source
                .authorization_evidence()
                .ok_or(DomainError::InvariantViolated {
                    reason: "accepted ceremony work has no authorization evidence",
                })?;
        let accepted = self
            .store
            .decision(&self.policy_id, source_evidence.decision_id())
            .await?
            .ok_or(DomainError::NotFound {
                what: "accepted_authorization_decision",
            })?;
        if accepted.evidence(source.occurred_at())? != *source_evidence {
            return Err(DomainError::InvariantViolated {
                reason: "accepted ceremony record evidence does not match its policy decision",
            });
        }
        let target = serde_json::to_vec(&(
            source.ceremony_id(),
            source.event_id(),
            source.record_hash(),
        ))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "accepted ceremony recovery target cannot be canonicalized",
        })?;
        let request = AuthorizationRequest::new(
            request_id,
            accepted.request().principal().clone(),
            AuthorizationAction::RecoverCeremonyChildren,
            accepted.request().scope().clone(),
            AuthorizationTargetDigest::for_bytes(&target),
        )
        .with_accepted_work(accepted.id().clone());

        for _ in 0..MAX_CONFLICT_RETRIES {
            let existing = self
                .store
                .decision_for_request(&self.policy_id, request.id())
                .await?;
            let snapshot =
                self.store
                    .load(&self.policy_id)
                    .await?
                    .ok_or(DomainError::NotFound {
                        what: "authorization_policy",
                    })?;
            let plan = snapshot.policy.decide_accepted_work(
                request.clone(),
                existing.as_ref(),
                &accepted,
                self.clock.now(),
                self.ttl,
            )?;
            let (decision, event) = plan.into_parts();
            if let Some(event) = event {
                match self
                    .store
                    .append(&self.policy_id, snapshot.version, vec![event])
                    .await?
                {
                    AuthorizationPolicyAppendOutcome::Appended { .. } => {}
                    AuthorizationPolicyAppendOutcome::Conflict { .. } => continue,
                }
            }
            let evidence = decision.evidence(self.clock.now())?;
            return AuthorizedOperation::new(request.principal().clone(), evidence);
        }
        Err(DomainError::Conflict {
            what: "authorization_policy",
        })
    }
}

impl std::fmt::Debug for ContinueAcceptedCeremonyWorkUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContinueAcceptedCeremonyWorkUseCase")
            .field("policy_id", &self.policy_id)
            .finish_non_exhaustive()
    }
}
