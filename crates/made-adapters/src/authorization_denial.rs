//! How an authorization denial is worded, once, for every transport.
//!
//! The gRPC service and the embedded MCP backend evaluate the same
//! policy and reach the same decision; a client must read the same
//! refusal whichever of them answered (the MCP parity session pins this
//! byte for byte). A bare decision id used to be the whole message, and
//! a session that read it had nothing to tell the person except that
//! something was refused. The facts a denial can state are the
//! decision's own: which action, for which principal, and why. How a
//! grant gets issued depends on the deployment, so both routes are
//! named rather than the one this process happens to have.

use made_core::value_objects::{
    AuthorizationAction, AuthorizationDecision, AuthorizationDenialReason, PrincipalId,
};

/// The message a denied operation is refused with, the same on every
/// surface that carries the decision.
#[must_use]
pub fn denial_message(decision: &AuthorizationDecision) -> String {
    let request = decision.request();
    denial_wording(
        decision.id().as_str(),
        request.action(),
        request.principal().id(),
        decision.denial_reason(),
    )
}

/// The wording from the decision's parts, so that every reason can be
/// read without minting a decision.
#[must_use]
pub fn denial_wording(
    decision_id: &str,
    action: AuthorizationAction,
    principal: &PrincipalId,
    reason: Option<AuthorizationDenialReason>,
) -> String {
    let action = serde_json::to_value(action)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let principal = principal.as_str();
    match reason {
        Some(AuthorizationDenialReason::NoMatchingGrant) => format!(
            "authorization decision {decision_id} denied `{action}`: principal `{principal}` \
             holds no live grant for it. A grant is issued outside this session: a person at \
             their own terminal runs `made-mcp grant <store> --profile core` (or `--actions \
             {action}`) against a local store; a policy administrator uses \
             `made_issue_authorization_grant` on a service. This session cannot grant itself; \
             `made_discover_capabilities` lists what it holds under `authorization` where the \
             server reads its policy."
        ),
        Some(AuthorizationDenialReason::ApprovalRequired) => format!(
            "authorization decision {decision_id} denied `{action}` for principal `{principal}`: \
             a separation rule requires another principal's approval of this exact operation \
             (`made_approve_authorization_operation`), presented as the approval decision id."
        ),
        Some(
            AuthorizationDenialReason::ApprovalInvalid
            | AuthorizationDenialReason::ApprovalIntentInvalid,
        ) => format!(
            "authorization decision {decision_id} denied `{action}` for principal `{principal}`: \
             the supplied approval does not cover this action, scope, target or principal."
        ),
        None => format!(
            "authorization decision {decision_id} denied `{action}` for principal `{principal}`."
        ),
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::AuthorizationPolicy;
    use made_core::value_objects::{
        AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
        AuthorizationDecisionTtl, AuthorizationDenialReason, AuthorizationPolicyId,
        AuthorizationRequest, AuthorizationRequestId, AuthorizationScope,
        AuthorizationTargetDigest, PrincipalId, PrincipalKind,
    };
    use time::OffsetDateTime;

    use super::{denial_message, denial_wording};

    fn host() -> PrincipalId {
        PrincipalId::new("laptop-host").unwrap()
    }

    #[test]
    fn a_missing_grant_names_the_action_the_principal_and_both_ways_to_grant() {
        let message = denial_wording(
            "d-1",
            AuthorizationAction::DesignCeremony,
            &host(),
            Some(AuthorizationDenialReason::NoMatchingGrant),
        );
        assert!(
            message.starts_with(
                "authorization decision d-1 denied `design_ceremony`: principal `laptop-host` \
                 holds no live grant"
            ),
            "{message}"
        );
        assert!(
            message.contains("made-mcp grant <store> --profile core"),
            "{message}"
        );
        assert!(message.contains("--actions design_ceremony"), "{message}");
        assert!(
            message.contains("made_issue_authorization_grant"),
            "{message}"
        );
        assert!(message.contains("cannot grant itself"), "{message}");
    }

    #[test]
    fn a_separation_rule_names_the_approval_and_a_bad_one_says_what_it_misses() {
        let required = denial_wording(
            "d-2",
            AuthorizationAction::CancelCeremony,
            &host(),
            Some(AuthorizationDenialReason::ApprovalRequired),
        );
        assert!(required.contains("separation rule"), "{required}");
        assert!(
            required.contains("made_approve_authorization_operation"),
            "{required}"
        );
        assert!(!required.contains("made-mcp grant"), "{required}");
        for reason in [
            AuthorizationDenialReason::ApprovalInvalid,
            AuthorizationDenialReason::ApprovalIntentInvalid,
        ] {
            let message = denial_wording(
                "d-3",
                AuthorizationAction::CancelCeremony,
                &host(),
                Some(reason),
            );
            assert!(
                message.contains("does not cover this action, scope, target or principal"),
                "{message}"
            );
        }
        let bare = denial_wording("d-4", AuthorizationAction::GetMetrics, &host(), None);
        assert_eq!(
            bare,
            "authorization decision d-4 denied `get_metrics` for principal `laptop-host`."
        );
    }

    /// The policy's own denial, worded from the decision it minted: an
    /// owner with no grant is refused a business action.
    #[test]
    fn a_decision_the_policy_minted_is_worded_from_its_own_facts() {
        let owner = AuthenticatedPrincipal::new(
            host(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(1);
        let mut policy = AuthorizationPolicy::empty();
        let opened = policy
            .decide_open(
                AuthorizationPolicyId::new("laptop-policy").unwrap(),
                owner.clone(),
                Vec::new(),
                now,
            )
            .unwrap()
            .expect("an empty policy opens");
        policy.apply(opened).unwrap();
        let request = AuthorizationRequest::new(
            AuthorizationRequestId::new("request-1").unwrap(),
            owner,
            AuthorizationAction::GetMetrics,
            AuthorizationScope::Global,
            AuthorizationTargetDigest::for_bytes(b"metrics"),
        );
        let plan = policy
            .decide_authorize(
                request,
                now,
                AuthorizationDecisionTtl::from_seconds(60).unwrap(),
            )
            .unwrap();
        let decision = plan.decision();
        assert_eq!(
            decision.denial_reason(),
            Some(AuthorizationDenialReason::NoMatchingGrant)
        );
        let message = denial_message(decision);
        assert!(
            message.starts_with(&format!(
                "authorization decision {} denied `get_metrics`: principal `laptop-host`",
                decision.id().as_str()
            )),
            "{message}"
        );
    }
}
