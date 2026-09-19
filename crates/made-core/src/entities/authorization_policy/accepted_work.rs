use time::OffsetDateTime;

use super::AuthorizationPolicy;
use crate::entities::AuthorizationPolicyEvent;
use crate::value_objects::{
    AuthorizationAction, AuthorizationDecision, AuthorizationDecisionKind,
    AuthorizationDecisionPlan, AuthorizationDecisionTtl, AuthorizationRequest,
};
use crate::DomainError;

pub(super) fn authority_is_valid(
    request: &AuthorizationRequest,
    accepted: &AuthorizationDecision,
) -> bool {
    request.accepted_work_decision_id() == Some(accepted.id())
        && accepted.kind() == AuthorizationDecisionKind::Allow
        && accepted.request().principal() == request.principal()
        && accepted.request().scope() == request.scope()
        && match request.action() {
            AuthorizationAction::RecoverCeremonyChildren => matches!(
                accepted.request().action(),
                AuthorizationAction::RunCeremony
                    | AuthorizationAction::RunCeremonyStep
                    | AuthorizationAction::PrepareCeremonyChildren
                    | AuthorizationAction::ApplyCeremonyTransition
                    | AuthorizationAction::RecoverCeremonyChildren
            ),
            AuthorizationAction::CompleteCeremonyStep => {
                matches!(
                    accepted.request().action(),
                    AuthorizationAction::ClaimCeremonyStep
                        | AuthorizationAction::RunCeremonyStep
                        | AuthorizationAction::RunCeremony
                )
            }
            AuthorizationAction::RenewCeremonyStepLease => matches!(
                accepted.request().action(),
                AuthorizationAction::ClaimCeremonyStep
                    | AuthorizationAction::RunCeremonyStep
                    | AuthorizationAction::RunCeremony
            ),
            _ => false,
        }
}

impl AuthorizationPolicy {
    /// Admit completion of work whose original authorization is sealed in a
    /// durable domain record. The current evidence remains tied to the exact
    /// principal, scope and admission decision even after grant revocation.
    pub fn decide_accepted_work(
        &self,
        request: AuthorizationRequest,
        existing: Option<&AuthorizationDecision>,
        accepted: &AuthorizationDecision,
        now: OffsetDateTime,
        ttl: AuthorizationDecisionTtl,
    ) -> Result<AuthorizationDecisionPlan, DomainError> {
        let policy_id = self.id.clone().ok_or(DomainError::NotFound {
            what: "authorization_policy",
        })?;
        if let Some(existing) = existing {
            if existing.request() == &request {
                return Ok(AuthorizationDecisionPlan::new(existing.clone(), None));
            }
            return Err(DomainError::Conflict {
                what: "authorization_request",
            });
        }
        if !authority_is_valid(&request, accepted) {
            return Err(DomainError::InvariantViolated {
                reason: "accepted-work authorization does not match its sealed admission",
            });
        }
        let valid_until =
            now.checked_add(ttl.duration())
                .ok_or(DomainError::InvariantViolated {
                    reason: "authorization decision expiry exceeds supported time range",
                })?;
        let decision =
            AuthorizationDecision::allow(request, self.version.next(), None, now, valid_until)?;
        Ok(AuthorizationDecisionPlan::new(
            decision.clone(),
            Some(AuthorizationPolicyEvent::DecisionRecorded {
                policy_id,
                decision,
            }),
        ))
    }
}
