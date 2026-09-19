use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AuthorizationPolicy, AuthorizationPolicyEvent};
use made_core::ports::{
    AuthorizationDecisionPage, AuthorizationPolicyAppendOutcome, AuthorizationPolicySnapshot,
    AuthorizationPolicyStorePort,
};
use made_core::value_objects::{
    AuthorizationDecision, AuthorizationDecisionId, AuthorizationDecisionPageLimit,
    AuthorizationPolicyId, AuthorizationPolicyVersion, AuthorizationRequestId,
};
use made_core::DomainError;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct InMemoryAuthorizationPolicyStore {
    streams: Arc<RwLock<BTreeMap<AuthorizationPolicyId, Vec<AuthorizationPolicyEvent>>>>,
}

impl InMemoryAuthorizationPolicyStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AuthorizationPolicyStorePort for InMemoryAuthorizationPolicyStore {
    async fn load(
        &self,
        policy_id: &AuthorizationPolicyId,
    ) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
        let streams = self.streams.read().await;
        streams
            .get(policy_id)
            .map(|events| snapshot(events))
            .transpose()
    }

    async fn append(
        &self,
        policy_id: &AuthorizationPolicyId,
        expected: AuthorizationPolicyVersion,
        events: Vec<AuthorizationPolicyEvent>,
    ) -> Result<AuthorizationPolicyAppendOutcome, DomainError> {
        validate_events(policy_id, &events)?;
        let mut streams = self.streams.write().await;
        let stored = streams.entry(policy_id.clone()).or_default();
        let actual = AuthorizationPolicyVersion::new(stored.len() as u64);
        if actual != expected {
            return Ok(AuthorizationPolicyAppendOutcome::Conflict { expected, actual });
        }
        let mut candidate = stored.clone();
        candidate.extend(events);
        AuthorizationPolicy::rehydrate(&candidate)?;
        *stored = candidate;
        Ok(AuthorizationPolicyAppendOutcome::Appended {
            version: AuthorizationPolicyVersion::new(stored.len() as u64),
        })
    }

    async fn decisions(
        &self,
        policy_id: &AuthorizationPolicyId,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        let streams = self.streams.read().await;
        let Some(events) = streams.get(policy_id) else {
            return Ok(AuthorizationDecisionPage::new(Vec::new()));
        };
        let policy = AuthorizationPolicy::rehydrate(events)?;
        let decisions = policy
            .decisions()
            .filter(|decision| after.is_none_or(|cursor| decision.id() > cursor))
            .take(limit.value())
            .cloned()
            .collect();
        Ok(AuthorizationDecisionPage::new(decisions))
    }

    async fn decision(
        &self,
        policy_id: &AuthorizationPolicyId,
        decision_id: &AuthorizationDecisionId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        let streams = self.streams.read().await;
        find_decision(streams.get(policy_id), |decision| {
            decision.id() == decision_id
        })
    }

    async fn decision_for_request(
        &self,
        policy_id: &AuthorizationPolicyId,
        request_id: &AuthorizationRequestId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        let streams = self.streams.read().await;
        find_decision(streams.get(policy_id), |decision| {
            decision.request().id() == request_id
        })
    }
}

fn find_decision(
    events: Option<&Vec<AuthorizationPolicyEvent>>,
    matches: impl Fn(&AuthorizationDecision) -> bool,
) -> Result<Option<AuthorizationDecision>, DomainError> {
    let Some(events) = events else {
        return Ok(None);
    };
    for event in events.iter().rev() {
        if let AuthorizationPolicyEvent::DecisionRecorded { decision, .. } = event {
            decision.validate()?;
            if matches(decision) {
                return Ok(Some(decision.clone()));
            }
        }
    }
    Ok(None)
}

fn snapshot(
    events: &[AuthorizationPolicyEvent],
) -> Result<AuthorizationPolicySnapshot, DomainError> {
    let policy = AuthorizationPolicy::rehydrate(events)?;
    Ok(AuthorizationPolicySnapshot {
        version: policy.version(),
        policy,
    })
}

fn validate_events(
    policy_id: &AuthorizationPolicyId,
    events: &[AuthorizationPolicyEvent],
) -> Result<(), DomainError> {
    if events.is_empty() {
        return Err(DomainError::EmptyCollection {
            field: "authorization_policy_events",
        });
    }
    if events.iter().any(|event| event.policy_id() != policy_id) {
        return Err(DomainError::InvariantViolated {
            reason: "authorization append contains an event for another policy",
        });
    }
    Ok(())
}
