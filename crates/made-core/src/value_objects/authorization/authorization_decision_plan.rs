use super::AuthorizationDecision;
use crate::entities::AuthorizationPolicyEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDecisionPlan {
    decision: AuthorizationDecision,
    event: Option<AuthorizationPolicyEvent>,
}

impl AuthorizationDecisionPlan {
    #[must_use]
    pub(crate) const fn new(
        decision: AuthorizationDecision,
        event: Option<AuthorizationPolicyEvent>,
    ) -> Self {
        Self { decision, event }
    }

    #[must_use]
    pub const fn decision(&self) -> &AuthorizationDecision {
        &self.decision
    }

    #[must_use]
    pub fn into_parts(self) -> (AuthorizationDecision, Option<AuthorizationPolicyEvent>) {
        (self.decision, self.event)
    }
}
