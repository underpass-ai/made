use std::collections::{BTreeMap, BTreeSet};

use time::OffsetDateTime;

use super::AuthorizationPolicyEvent;
use crate::value_objects::{
    AuthenticatedPrincipal, AuthorizationAction, AuthorizationDecision, AuthorizationDecisionId,
    AuthorizationDecisionKind, AuthorizationDecisionPlan, AuthorizationDecisionTtl,
    AuthorizationDenialReason, AuthorizationGrant, AuthorizationGrantId, AuthorizationPolicyId,
    AuthorizationPolicyVersion, AuthorizationRequest, AuthorizationRequestId,
    AuthorizationRevocation, AuthorizationRevocationReason, AuthorizationScope, PrincipalId,
    PrincipalKind, SeparationRule,
};
use crate::DomainError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthorizationPolicy {
    id: Option<AuthorizationPolicyId>,
    owner: Option<AuthenticatedPrincipal>,
    version: AuthorizationPolicyVersion,
    grants: BTreeMap<AuthorizationGrantId, AuthorizationGrant>,
    revoked: BTreeSet<AuthorizationGrantId>,
    separation_rules: BTreeMap<AuthorizationAction, SeparationRule>,
    decisions_by_request: BTreeMap<AuthorizationRequestId, AuthorizationDecision>,
    decisions_by_id: BTreeMap<AuthorizationDecisionId, AuthorizationDecision>,
}

impl AuthorizationPolicy {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn rehydrate(events: &[AuthorizationPolicyEvent]) -> Result<Self, DomainError> {
        let mut policy = Self::empty();
        for event in events {
            policy.apply(event.clone())?;
        }
        Ok(policy)
    }
    #[must_use]
    pub const fn version(&self) -> AuthorizationPolicyVersion {
        self.version
    }
    #[must_use]
    pub fn id(&self) -> Option<&AuthorizationPolicyId> {
        self.id.as_ref()
    }
    #[must_use]
    pub fn owner(&self) -> Option<&AuthenticatedPrincipal> {
        self.owner.as_ref()
    }
    pub fn decisions(&self) -> impl Iterator<Item = &AuthorizationDecision> {
        self.decisions_by_id.values()
    }

    pub fn decide_open(
        &self,
        policy_id: AuthorizationPolicyId,
        owner: AuthenticatedPrincipal,
        separation_rules: Vec<SeparationRule>,
        opened_at: OffsetDateTime,
    ) -> Result<Option<AuthorizationPolicyEvent>, DomainError> {
        owner.validate()?;
        if owner.kind() != PrincipalKind::TrustedHost {
            return Err(DomainError::InvariantViolated {
                reason: "authorization policy owner must be an explicit trusted host",
            });
        }
        let proposed_rules = separation_rule_map(separation_rules.clone())?;
        match (&self.id, &self.owner) {
            (None, None) => Ok(Some(AuthorizationPolicyEvent::Opened {
                policy_id,
                owner,
                separation_rules,
                opened_at,
            })),
            (Some(id), Some(stored_owner))
                if id == &policy_id
                    && stored_owner == &owner
                    && self.separation_rules == proposed_rules =>
            {
                Ok(None)
            }
            _ => Err(DomainError::Conflict {
                what: "authorization_policy",
            }),
        }
    }

    pub fn decide_authorize(
        &self,
        request: AuthorizationRequest,
        now: OffsetDateTime,
        ttl: AuthorizationDecisionTtl,
    ) -> Result<AuthorizationDecisionPlan, DomainError> {
        let policy_id = self.id.clone().ok_or(DomainError::NotFound {
            what: "authorization_policy",
        })?;
        if let Some(existing) = self.decisions_by_request.get(request.id()) {
            if existing.request() == &request {
                return Ok(AuthorizationDecisionPlan::new(existing.clone(), None));
            }
            return Err(DomainError::Conflict {
                what: "authorization_request",
            });
        }

        let grant = self.matching_grant(&request, now);
        let owner = self
            .owner
            .as_ref()
            .is_some_and(|value| value.id() == request.principal().id());
        let denial = if !owner && grant.is_none() {
            Some(AuthorizationDenialReason::NoMatchingGrant)
        } else {
            self.separation_denial(&request, now)
        };
        let requested_until =
            now.checked_add(ttl.duration())
                .ok_or(DomainError::InvariantViolated {
                    reason: "authorization decision expiry exceeds supported time range",
                })?;
        let valid_until = grant
            .and_then(AuthorizationGrant::valid_until)
            .map_or(requested_until, |grant_until| {
                grant_until.min(requested_until)
            });
        let decision = if let Some(reason) = denial {
            AuthorizationDecision::deny(request, self.version.next(), reason, now, requested_until)?
        } else {
            AuthorizationDecision::allow(
                request,
                self.version.next(),
                grant.map(|value| value.id().clone()),
                now,
                valid_until,
            )?
        };
        let event = AuthorizationPolicyEvent::DecisionRecorded {
            policy_id,
            decision: decision.clone(),
        };
        Ok(AuthorizationDecisionPlan::new(decision, Some(event)))
    }

    pub fn decide_issue(
        &self,
        issuer: &PrincipalId,
        grant: AuthorizationGrant,
        now: OffsetDateTime,
    ) -> Result<Option<AuthorizationPolicyEvent>, DomainError> {
        grant.validate()?;
        let policy_id = self.id.clone().ok_or(DomainError::NotFound {
            what: "authorization_policy",
        })?;
        if grant.issued_by() != issuer {
            return Err(DomainError::InvariantViolated {
                reason: "authorization grant issuer must be the authenticated principal",
            });
        }
        if let Some(existing) = self.grants.get(grant.id()) {
            return if existing == &grant {
                Ok(None)
            } else {
                Err(DomainError::Conflict {
                    what: "authorization_grant",
                })
            };
        }
        let authority_is_valid = if self.is_owner(issuer) {
            grant.parent_grant_id().is_none()
        } else {
            self.may_delegate(issuer, &grant, now)
        };
        if !authority_is_valid {
            return Err(DomainError::InvariantViolated {
                reason: "principal cannot delegate the requested authorization grant",
            });
        }
        Ok(Some(AuthorizationPolicyEvent::GrantIssued {
            policy_id,
            grant,
            issued_at: now,
        }))
    }

    pub fn decide_revoke(
        &self,
        issuer: &PrincipalId,
        grant_id: &AuthorizationGrantId,
        reason: AuthorizationRevocationReason,
        now: OffsetDateTime,
    ) -> Result<Option<AuthorizationPolicyEvent>, DomainError> {
        let policy_id = self.id.clone().ok_or(DomainError::NotFound {
            what: "authorization_policy",
        })?;
        let grant = self.grants.get(grant_id).ok_or(DomainError::NotFound {
            what: "authorization_grant",
        })?;
        if self.revoked.contains(grant_id) {
            return Ok(None);
        }
        let allowed = self.is_owner(issuer)
            || grant.issued_by() == issuer
            || self.permits(
                issuer,
                AuthorizationAction::RevokeAuthorizationGrant,
                grant.scope(),
                now,
            );
        if !allowed {
            return Err(DomainError::InvariantViolated {
                reason: "principal cannot revoke the authorization grant",
            });
        }
        Ok(Some(AuthorizationPolicyEvent::GrantRevoked {
            policy_id,
            revocation: AuthorizationRevocation::new(grant_id.clone(), issuer.clone(), reason, now),
        }))
    }

    #[must_use]
    pub fn permits(
        &self,
        principal: &PrincipalId,
        action: AuthorizationAction,
        scope: &AuthorizationScope,
        now: OffsetDateTime,
    ) -> bool {
        self.is_owner(principal)
            || self.grants.values().any(|grant| {
                grant.grantee() == principal
                    && grant.permits(action, scope, now)
                    && self.grant_chain_is_live(grant, now)
            })
    }

    pub fn apply(&mut self, event: AuthorizationPolicyEvent) -> Result<(), DomainError> {
        if let Some(id) = &self.id {
            if event.policy_id() != id {
                return Err(DomainError::InvariantViolated {
                    reason: "authorization event belongs to another policy",
                });
            }
        }
        match event {
            AuthorizationPolicyEvent::Opened {
                policy_id,
                owner,
                separation_rules,
                ..
            } => {
                if self.id.is_some() {
                    return Err(DomainError::AlreadyExists {
                        what: "authorization_policy",
                    });
                }
                owner.validate()?;
                if owner.kind() != PrincipalKind::TrustedHost {
                    return Err(DomainError::InvariantViolated {
                        reason: "authorization policy owner must be an explicit trusted host",
                    });
                }
                let mapped_rules = separation_rule_map(separation_rules)?;
                self.id = Some(policy_id);
                self.owner = Some(owner);
                self.separation_rules = mapped_rules;
            }
            AuthorizationPolicyEvent::GrantIssued {
                grant, issued_at, ..
            } => {
                if self.id.is_none() {
                    return Err(DomainError::NotFound {
                        what: "authorization_policy",
                    });
                }
                if self.grants.contains_key(grant.id()) {
                    return Err(DomainError::AlreadyExists {
                        what: "authorization_grant",
                    });
                }
                grant.validate()?;
                let authority_is_valid = if self.is_owner(grant.issued_by()) {
                    grant.parent_grant_id().is_none()
                } else {
                    self.may_delegate(grant.issued_by(), &grant, issued_at)
                };
                if !authority_is_valid {
                    return Err(DomainError::InvariantViolated {
                        reason: "stored authorization grant has invalid delegation authority",
                    });
                }
                self.grants.insert(grant.id().clone(), grant);
            }
            AuthorizationPolicyEvent::GrantRevoked { revocation, .. } => {
                if !self.grants.contains_key(revocation.grant_id()) {
                    return Err(DomainError::NotFound {
                        what: "authorization_grant",
                    });
                }
                if self.revoked.contains(revocation.grant_id()) {
                    return Err(DomainError::AlreadyExists {
                        what: "authorization_revocation",
                    });
                }
                self.revoked.insert(revocation.grant_id().clone());
            }
            AuthorizationPolicyEvent::DecisionRecorded { decision, .. } => {
                decision.validate()?;
                if decision.policy_version() != self.version.next() {
                    return Err(DomainError::InvariantViolated {
                        reason: "authorization decision carries the wrong policy version",
                    });
                }
                if self
                    .decisions_by_request
                    .contains_key(decision.request().id())
                    || self.decisions_by_id.contains_key(decision.id())
                {
                    return Err(DomainError::AlreadyExists {
                        what: "authorization_decision",
                    });
                }
                if !self.decision_authority_is_valid(&decision) {
                    return Err(DomainError::InvariantViolated {
                        reason: "stored authorization decision contradicts the active policy",
                    });
                }
                self.decisions_by_request
                    .insert(decision.request().id().clone(), decision.clone());
                self.decisions_by_id.insert(decision.id().clone(), decision);
            }
        }
        self.version = self.version.next();
        Ok(())
    }

    fn may_delegate(
        &self,
        issuer: &PrincipalId,
        child: &AuthorizationGrant,
        now: OffsetDateTime,
    ) -> bool {
        let Some(parent_id) = child.parent_grant_id() else {
            return false;
        };
        self.grants.get(parent_id).is_some_and(|parent| {
            parent.grantee() == issuer
                && self.grant_chain_is_live(parent, now)
                && parent
                    .actions()
                    .contains(&AuthorizationAction::IssueAuthorizationGrant)
                && child.actions().is_subset(parent.actions())
                && parent.scope().covers(child.scope())
                && parent.valid_from() <= child.valid_from()
                && validity_contains(parent.valid_until(), child.valid_until())
                && parent.delegation_depth().value() > child.delegation_depth().value()
        })
    }

    fn matching_grant(
        &self,
        request: &AuthorizationRequest,
        now: OffsetDateTime,
    ) -> Option<&AuthorizationGrant> {
        self.grants.values().find(|grant| {
            grant.grantee() == request.principal().id()
                && grant.permits(request.action(), request.scope(), now)
                && self.grant_chain_is_live(grant, now)
        })
    }

    fn grant_chain_is_live(&self, grant: &AuthorizationGrant, now: OffsetDateTime) -> bool {
        if self.revoked.contains(grant.id()) || !grant.is_active_at(now) {
            return false;
        }
        let Some(parent_id) = grant.parent_grant_id() else {
            return self.is_owner(grant.issued_by());
        };
        self.grants.get(parent_id).is_some_and(|parent| {
            parent.grantee() == grant.issued_by() && self.grant_chain_is_live(parent, now)
        })
    }

    fn is_owner(&self, principal: &PrincipalId) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| owner.id() == principal)
    }

    fn separation_denial(
        &self,
        request: &AuthorizationRequest,
        now: OffsetDateTime,
    ) -> Option<AuthorizationDenialReason> {
        let rule = self.separation_rules.get(&request.action())?;
        let Some(approval_id) = request.approval_decision_id() else {
            return Some(AuthorizationDenialReason::ApprovalRequired);
        };
        let valid = self
            .decisions_by_id
            .get(approval_id)
            .is_some_and(|approval| {
                approval.kind() == AuthorizationDecisionKind::Allow
                    && approval.is_live_at(now)
                    && approval.request().action() == rule.approval_action()
                    && approval.request().principal().id() != request.principal().id()
                    && approval.request().scope().covers(request.scope())
                    && approval.request().target_digest() == request.target_digest()
            });
        (!valid).then_some(AuthorizationDenialReason::ApprovalInvalid)
    }

    fn decision_authority_is_valid(&self, decision: &AuthorizationDecision) -> bool {
        let request = decision.request();
        let now = decision.decided_at();
        if decision.valid_until() - now > time::Duration::seconds(300) {
            return false;
        }
        let owner = self.is_owner(request.principal().id());
        let grant = decision
            .grant_id()
            .and_then(|grant_id| self.grants.get(grant_id));
        let granted = owner && decision.grant_id().is_none()
            || grant.is_some_and(|grant| {
                grant.grantee() == request.principal().id()
                    && grant.permits(request.action(), request.scope(), now)
                    && self.grant_chain_is_live(grant, now)
                    && grant
                        .valid_until()
                        .is_none_or(|until| decision.valid_until() <= until)
            });
        let separation_denial = self.separation_denial(request, now);
        match decision.kind() {
            AuthorizationDecisionKind::Allow => granted && separation_denial.is_none(),
            AuthorizationDecisionKind::Deny => {
                let expected = if !owner && self.matching_grant(request, now).is_none() {
                    Some(AuthorizationDenialReason::NoMatchingGrant)
                } else {
                    separation_denial
                };
                decision.grant_id().is_none() && decision.denial_reason() == expected
            }
        }
    }
}

fn validity_contains(parent: Option<OffsetDateTime>, child: Option<OffsetDateTime>) -> bool {
    match (parent, child) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(parent), Some(child)) => child <= parent,
    }
}

fn separation_rule_map(
    rules: Vec<SeparationRule>,
) -> Result<BTreeMap<AuthorizationAction, SeparationRule>, DomainError> {
    let count = rules.len();
    let mapped = rules
        .into_iter()
        .map(|rule| (rule.execution_action(), rule))
        .collect::<BTreeMap<_, _>>();
    if mapped.len() != count {
        return Err(DomainError::Conflict {
            what: "authorization_separation_rule",
        });
    }
    Ok(mapped)
}
