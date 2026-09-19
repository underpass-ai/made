use std::sync::Arc;

use made_core::ports::{AuthorizationPolicyAppendOutcome, AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthorizationDecision, AuthorizationDecisionKind, AuthorizationDecisionTtl,
    AuthorizationPolicyId, AuthorizationRequest,
};
use made_core::DomainError;

const MAX_CONFLICT_RETRIES: usize = 16;

use super::AuthorizationGateOutcome;

#[derive(Clone)]
pub struct AuthorizeOperationUseCase {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<dyn ClockPort>,
    ttl: AuthorizationDecisionTtl,
}

impl std::fmt::Debug for AuthorizeOperationUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizeOperationUseCase")
            .field("policy_id", &self.policy_id)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl AuthorizeOperationUseCase {
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

    pub async fn execute(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationGateOutcome, DomainError> {
        let decision = self.decide(request).await?;
        if decision.kind() == AuthorizationDecisionKind::Deny {
            return Ok(AuthorizationGateOutcome::Denied { decision });
        }
        match decision.evidence(self.clock.now()) {
            Ok(evidence) => Ok(AuthorizationGateOutcome::Allowed { decision, evidence }),
            Err(_) => Ok(AuthorizationGateOutcome::Expired { decision }),
        }
    }

    async fn decide(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, DomainError> {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot =
                self.store
                    .load(&self.policy_id)
                    .await?
                    .ok_or(DomainError::NotFound {
                        what: "authorization_policy",
                    })?;
            let plan =
                snapshot
                    .policy
                    .decide_authorize(request.clone(), self.clock.now(), self.ttl)?;
            let (decision, event) = plan.into_parts();
            let Some(event) = event else {
                return Ok(decision);
            };
            match self
                .store
                .append(&self.policy_id, snapshot.version, vec![event])
                .await?
            {
                AuthorizationPolicyAppendOutcome::Appended { .. } => return Ok(decision),
                AuthorizationPolicyAppendOutcome::Conflict { .. } => {}
            }
        }
        Err(DomainError::Conflict {
            what: "authorization_policy",
        })
    }
}
