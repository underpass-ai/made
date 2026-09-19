use time::OffsetDateTime;

use crate::value_objects::{
    AuthorizationDecision, AuthorizationDecisionKind, AuthorizationDenialReason,
    AuthorizationRequest,
};

use super::AuthorizationPolicy;

impl AuthorizationPolicy {
    pub(super) fn separation_denial(
        &self,
        request: &AuthorizationRequest,
        approval: Option<&AuthorizationDecision>,
        now: OffsetDateTime,
    ) -> Option<AuthorizationDenialReason> {
        let rule = self.separation_rules.get(&request.action())?;
        let Some(approval_id) = request.approval_decision_id() else {
            return Some(AuthorizationDenialReason::ApprovalRequired);
        };
        let valid = approval
            .filter(|approval| approval.id() == approval_id)
            .is_some_and(|approval| {
                approval.kind() == AuthorizationDecisionKind::Allow
                    && approval.is_live_at(now)
                    && self.decision_grant_is_live(approval, now)
                    && approval.request().action() == rule.approval_action()
                    && approval.request().approved_action() == Some(request.action())
                    && approval.request().principal().id() != request.principal().id()
                    && approval.request().scope().covers(request.scope())
                    && approval.request().target_digest() == request.target_digest()
            });
        (!valid).then_some(AuthorizationDenialReason::ApprovalInvalid)
    }

    pub(super) fn approval_intent_denial(
        &self,
        request: &AuthorizationRequest,
    ) -> Option<AuthorizationDenialReason> {
        let execution_action = request.approved_action()?;
        let valid = self
            .separation_rules
            .get(&execution_action)
            .is_some_and(|rule| rule.approval_action() == request.action());
        (!valid).then_some(AuthorizationDenialReason::ApprovalIntentInvalid)
    }
}
