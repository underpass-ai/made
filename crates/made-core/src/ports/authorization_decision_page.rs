use crate::value_objects::AuthorizationDecision;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDecisionPage {
    decisions: Vec<AuthorizationDecision>,
}

impl AuthorizationDecisionPage {
    #[must_use]
    pub const fn new(decisions: Vec<AuthorizationDecision>) -> Self {
        Self { decisions }
    }

    #[must_use]
    pub fn decisions(&self) -> &[AuthorizationDecision] {
        &self.decisions
    }

    #[must_use]
    pub fn into_decisions(self) -> Vec<AuthorizationDecision> {
        self.decisions
    }
}
