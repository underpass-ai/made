use std::sync::Arc;

use made_core::entities::AuthorizationPolicy;
use made_core::ports::{AuthorizationPolicyAppendOutcome, AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthorizationGrant, AuthorizationGrantId, AuthorizationPolicyId,
    AuthorizationRevocationReason, SeparationRule,
};
use made_core::DomainError;

use super::AuthorizationMutationOutcome;

const MAX_CONFLICT_RETRIES: usize = 16;

#[derive(Clone)]
pub struct AuthorizationPolicyAdministrationService {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AuthorizationPolicyAdministrationService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizationPolicyAdministrationService")
            .field("policy_id", &self.policy_id)
            .finish_non_exhaustive()
    }
}

impl AuthorizationPolicyAdministrationService {
    #[must_use]
    pub fn new(
        policy_id: AuthorizationPolicyId,
        store: Arc<dyn AuthorizationPolicyStorePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            policy_id,
            store,
            clock,
        }
    }

    pub async fn open(
        &self,
        owner: AuthenticatedPrincipal,
        separation_rules: Vec<SeparationRule>,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot = self.store.load(&self.policy_id).await?;
            let policy = snapshot
                .as_ref()
                .map_or_else(AuthorizationPolicy::empty, |value| value.policy.clone());
            let Some(event) = policy.decide_open(
                self.policy_id.clone(),
                owner.clone(),
                separation_rules.clone(),
                self.clock.now(),
            )?
            else {
                return Ok(AuthorizationMutationOutcome::Existing {
                    version: policy.version(),
                });
            };
            if let Some(outcome) = append_outcome(
                self.store
                    .append(&self.policy_id, policy.version(), vec![event])
                    .await?,
            ) {
                return Ok(outcome);
            }
        }
        conflict()
    }

    pub async fn issue(
        &self,
        principal: &AuthenticatedPrincipal,
        grant: AuthorizationGrant,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        self.mutate(principal, |policy, issuer, now| {
            policy.decide_issue(issuer, grant.clone(), now)
        })
        .await
    }

    pub async fn revoke(
        &self,
        principal: &AuthenticatedPrincipal,
        grant_id: &AuthorizationGrantId,
        reason: AuthorizationRevocationReason,
    ) -> Result<AuthorizationMutationOutcome, DomainError> {
        self.mutate(principal, |policy, issuer, now| {
            policy.decide_revoke(issuer, grant_id, reason.clone(), now)
        })
        .await
    }

    async fn mutate<F>(
        &self,
        issuer: &AuthenticatedPrincipal,
        decide: F,
    ) -> Result<AuthorizationMutationOutcome, DomainError>
    where
        F: Fn(
            &AuthorizationPolicy,
            &AuthenticatedPrincipal,
            time::OffsetDateTime,
        )
            -> Result<Option<made_core::entities::AuthorizationPolicyEvent>, DomainError>,
    {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot =
                self.store
                    .load(&self.policy_id)
                    .await?
                    .ok_or(DomainError::NotFound {
                        what: "authorization_policy",
                    })?;
            let Some(event) = decide(&snapshot.policy, issuer, self.clock.now())? else {
                return Ok(AuthorizationMutationOutcome::Existing {
                    version: snapshot.version,
                });
            };
            if let Some(outcome) = append_outcome(
                self.store
                    .append(&self.policy_id, snapshot.version, vec![event])
                    .await?,
            ) {
                return Ok(outcome);
            }
        }
        conflict()
    }
}

fn append_outcome(
    outcome: AuthorizationPolicyAppendOutcome,
) -> Option<AuthorizationMutationOutcome> {
    match outcome {
        AuthorizationPolicyAppendOutcome::Appended { version } => {
            Some(AuthorizationMutationOutcome::Applied { version })
        }
        AuthorizationPolicyAppendOutcome::Conflict { .. } => None,
    }
}

fn conflict() -> Result<AuthorizationMutationOutcome, DomainError> {
    Err(DomainError::Conflict {
        what: "authorization_policy",
    })
}
