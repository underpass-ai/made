use time::{macros::datetime, Duration, OffsetDateTime};

use super::{AuthorizationPolicy, AuthorizationPolicyEvent};
use crate::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionKind,
    AuthorizationDecisionTtl, AuthorizationDenialReason, AuthorizationGrant, AuthorizationGrantId,
    AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationRequest, AuthorizationRequestId,
    AuthorizationRevocation, AuthorizationRevocationReason, AuthorizationScope,
    AuthorizationTargetDigest, DelegationDepth, PrincipalId, PrincipalKind, SeparationRule,
};
use crate::DomainError;

const NOW: OffsetDateTime = datetime!(2026-09-19 12:00:00 UTC);

#[test]
fn policy_owner_must_be_an_explicit_trusted_host() {
    let policy = AuthorizationPolicy::empty();
    let error = policy
        .decide_open(policy_id(), human("owner"), Vec::new(), NOW)
        .unwrap_err();
    assert_eq!(
        error,
        DomainError::InvariantViolated {
            reason: "authorization policy owner must be an explicit trusted host"
        }
    );
}

#[test]
fn matching_owner_id_with_another_authenticated_identity_has_no_owner_authority() {
    let policy = opened_policy(Vec::new());
    let outcome = policy
        .decide_authorize(
            request(
                "owner-imposter",
                human("trusted-host"),
                AuthorizationAction::PauseCeremony,
                b"ceremony",
            ),
            NOW,
            ttl(),
        )
        .unwrap();
    assert_eq!(outcome.decision().kind(), AuthorizationDecisionKind::Deny);
    assert_eq!(
        outcome.decision().denial_reason(),
        Some(AuthorizationDenialReason::NoMatchingGrant)
    );
}

#[test]
fn policy_owner_administers_policy_but_needs_grants_for_business_actions() {
    let policy = opened_policy(Vec::new());
    let business = policy
        .decide_authorize(
            request(
                "owner-business",
                trusted_host(),
                AuthorizationAction::PauseCeremony,
                b"ceremony",
            ),
            NOW,
            ttl(),
        )
        .unwrap();
    assert_eq!(business.decision().kind(), AuthorizationDecisionKind::Deny);

    let administration = policy
        .decide_authorize(
            request(
                "owner-administration",
                trusted_host(),
                AuthorizationAction::IssueAuthorizationGrant,
                b"policy",
            ),
            NOW,
            ttl(),
        )
        .unwrap();
    assert_eq!(
        administration.decision().kind(),
        AuthorizationDecisionKind::Allow
    );
}

#[test]
fn revoked_grant_can_only_continue_the_same_principal_and_sealed_work() {
    let mut policy = opened_policy(Vec::new());
    let executor = worker("executor");
    let grant = grant(
        "run-grant",
        executor.id().clone(),
        [AuthorizationAction::RunCeremony],
        DelegationDepth::none(),
        trusted_host(),
        None,
    );
    let grant_id = grant.id().clone();
    issue(&mut policy, &trusted_host(), grant);
    let admitted = policy
        .decide_authorize(
            request(
                "accepted-run",
                executor.clone(),
                AuthorizationAction::RunCeremony,
                b"run",
            ),
            NOW,
            ttl(),
        )
        .unwrap();
    let (accepted, event) = admitted.into_parts();
    policy.apply(event.unwrap()).unwrap();
    let revoked = policy
        .decide_revoke(
            &trusted_host(),
            &grant_id,
            AuthorizationRevocationReason::new("stop new work").unwrap(),
            NOW + Duration::seconds(1),
        )
        .unwrap()
        .unwrap();
    policy.apply(revoked).unwrap();

    let continuation = request(
        "drain-run",
        executor.clone(),
        AuthorizationAction::RecoverCeremonyChildren,
        b"sealed-effect",
    )
    .with_accepted_work(accepted.id().clone());
    let plan = policy
        .decide_accepted_work(
            continuation.clone(),
            None,
            &accepted,
            NOW + Duration::seconds(2),
            ttl(),
        )
        .unwrap();
    assert_eq!(plan.decision().kind(), AuthorizationDecisionKind::Allow);
    policy.apply(plan.into_parts().1.unwrap()).unwrap();

    let wrong_principal = request(
        "stolen-drain",
        worker("other"),
        AuthorizationAction::RecoverCeremonyChildren,
        b"sealed-effect",
    )
    .with_accepted_work(accepted.id().clone());
    assert!(policy
        .decide_accepted_work(
            wrong_principal,
            None,
            &accepted,
            NOW + Duration::seconds(2),
            ttl(),
        )
        .is_err());
}

#[test]
fn sealed_child_completion_transition_can_drive_recovery() {
    let mut policy = opened_policy(Vec::new());
    let executor = worker("executor");
    let transition_grant = grant(
        "transition-grant",
        executor.id().clone(),
        [AuthorizationAction::ApplyCeremonyTransition],
        DelegationDepth::none(),
        trusted_host(),
        None,
    );
    issue(&mut policy, &trusted_host(), transition_grant);
    let (accepted, event) = policy
        .decide_authorize(
            request(
                "accepted-terminal-transition",
                executor.clone(),
                AuthorizationAction::ApplyCeremonyTransition,
                b"child-terminal",
            ),
            NOW,
            ttl(),
        )
        .unwrap()
        .into_parts();
    policy.apply(event.unwrap()).unwrap();
    let recovery = request(
        "recover-terminal-child",
        executor,
        AuthorizationAction::RecoverCeremonyChildren,
        b"sealed-terminal-event",
    )
    .with_accepted_work(accepted.id().clone());

    assert_eq!(
        policy
            .decide_accepted_work(recovery, None, &accepted, NOW + Duration::seconds(1), ttl(),)
            .unwrap()
            .decision()
            .kind(),
        AuthorizationDecisionKind::Allow
    );
}

#[test]
fn revoked_claim_can_only_complete_as_the_same_authenticated_principal() {
    let mut policy = opened_policy(Vec::new());
    let executor = worker("executor");
    let claim_grant = grant(
        "claim-grant",
        executor.id().clone(),
        [AuthorizationAction::ClaimCeremonyStep],
        DelegationDepth::none(),
        trusted_host(),
        None,
    );
    let grant_id = claim_grant.id().clone();
    issue(&mut policy, &trusted_host(), claim_grant);
    let (accepted, event) = policy
        .decide_authorize(
            request(
                "accepted-claim",
                executor.clone(),
                AuthorizationAction::ClaimCeremonyStep,
                b"claim",
            ),
            NOW,
            ttl(),
        )
        .unwrap()
        .into_parts();
    policy.apply(event.unwrap()).unwrap();
    let revoke = policy
        .decide_revoke(
            &trusted_host(),
            &grant_id,
            AuthorizationRevocationReason::new("drain accepted claim only").unwrap(),
            NOW + Duration::seconds(1),
        )
        .unwrap()
        .unwrap();
    policy.apply(revoke).unwrap();

    let completion = request(
        "complete-accepted-claim",
        executor.clone(),
        AuthorizationAction::CompleteCeremonyStep,
        b"result",
    )
    .with_accepted_work(accepted.id().clone());
    assert_eq!(
        policy
            .decide_accepted_work(
                completion,
                None,
                &accepted,
                NOW + Duration::seconds(2),
                ttl(),
            )
            .unwrap()
            .decision()
            .kind(),
        AuthorizationDecisionKind::Allow
    );

    let same_id_different_identity = request(
        "complete-as-human",
        human("executor"),
        AuthorizationAction::CompleteCeremonyStep,
        b"result",
    )
    .with_accepted_work(accepted.id().clone());
    assert!(policy
        .decide_accepted_work(
            same_id_different_identity,
            None,
            &accepted,
            NOW + Duration::seconds(2),
            ttl(),
        )
        .is_err());
}

#[test]
fn forged_revocation_is_rejected_without_changing_rehydrated_state() {
    let mut policy = opened_policy(Vec::new());
    let grant = grant(
        "protected-grant",
        worker("worker").id().clone(),
        [AuthorizationAction::PauseCeremony],
        DelegationDepth::none(),
        trusted_host(),
        None,
    );
    let grant_id = grant.id().clone();
    issue(&mut policy, &trusted_host(), grant);
    let before = policy.clone();
    let forged = AuthorizationPolicyEvent::GrantRevoked {
        policy_id: policy_id(),
        revocation: AuthorizationRevocation::new(
            grant_id,
            service("attacker"),
            AuthorizationRevocationReason::new("forged").unwrap(),
            NOW + Duration::seconds(1),
        ),
    };

    assert_eq!(
        policy.apply(forged).unwrap_err(),
        DomainError::InvariantViolated {
            reason: "stored authorization revocation has invalid authority"
        }
    );
    assert_eq!(policy, before);
}

#[test]
fn exact_request_retry_returns_the_sealed_decision_and_changed_target_conflicts() {
    let mut policy = opened_policy(Vec::new());
    issue(
        &mut policy,
        &trusted_host(),
        grant(
            "grant-1",
            worker("worker").id().clone(),
            [AuthorizationAction::ClaimCeremonyStep],
            DelegationDepth::none(),
            trusted_host(),
            None,
        ),
    );

    let original_request = request(
        "request-1",
        worker("worker"),
        AuthorizationAction::ClaimCeremonyStep,
        b"claim-one",
    );
    let first = policy
        .decide_authorize(original_request.clone(), NOW, ttl())
        .unwrap();
    let (decision, event) = first.into_parts();
    assert_eq!(decision.kind(), AuthorizationDecisionKind::Allow);
    apply(&mut policy, event.unwrap());

    let repeated = policy
        .decide_authorize(original_request, NOW, ttl())
        .unwrap();
    assert_eq!(repeated.decision(), &decision);
    assert!(repeated.into_parts().1.is_none());

    let changed = request(
        "request-1",
        worker("worker"),
        AuthorizationAction::ClaimCeremonyStep,
        b"claim-two",
    );
    assert_eq!(
        policy.decide_authorize(changed, NOW, ttl()).unwrap_err(),
        DomainError::Conflict {
            what: "authorization_request"
        }
    );
}

#[test]
fn revoke_winner_blocks_new_authorization_but_does_not_rewrite_prior_allow() {
    let mut policy = opened_policy(Vec::new());
    let grant = grant(
        "grant-1",
        worker("worker").id().clone(),
        [AuthorizationAction::CompleteCeremonyStep],
        DelegationDepth::none(),
        trusted_host(),
        None,
    );
    let grant_id = grant.id().clone();
    issue(&mut policy, &trusted_host(), grant);
    let admitted = request(
        "admitted",
        worker("worker"),
        AuthorizationAction::CompleteCeremonyStep,
        b"fence-one",
    );
    let (allow, event) = policy
        .decide_authorize(admitted.clone(), NOW, ttl())
        .unwrap()
        .into_parts();
    apply(&mut policy, event.unwrap());
    revoke(
        &mut policy,
        &trusted_host(),
        &grant_id,
        NOW + Duration::seconds(1),
    );

    let repeated = policy.decide_authorize(admitted, NOW, ttl()).unwrap();
    assert_eq!(repeated.decision(), &allow);
    let denied = policy
        .decide_authorize(
            request(
                "new-request",
                worker("worker"),
                AuthorizationAction::CompleteCeremonyStep,
                b"fence-two",
            ),
            NOW + Duration::seconds(1),
            ttl(),
        )
        .unwrap();
    assert_eq!(denied.decision().kind(), AuthorizationDecisionKind::Deny);
    assert_eq!(
        denied.decision().denial_reason(),
        Some(AuthorizationDenialReason::NoMatchingGrant)
    );
}

#[test]
fn delegation_cannot_expand_actions_validity_or_depth() {
    let mut policy = opened_policy(Vec::new());
    let parent_until = NOW + Duration::minutes(10);
    let parent = grant(
        "parent",
        service("delegate").id().clone(),
        [
            AuthorizationAction::IssueAuthorizationGrant,
            AuthorizationAction::PauseCeremony,
        ],
        DelegationDepth::new(1).unwrap(),
        trusted_host(),
        Some(parent_until),
    );
    issue(&mut policy, &trusted_host(), parent);

    let child = delegated_grant(
        "child",
        worker("worker").id().clone(),
        [AuthorizationAction::PauseCeremony],
        DelegationDepth::none(),
        service("delegate"),
        AuthorizationGrantId::new("parent").unwrap(),
        Some(parent_until),
    );
    assert!(policy
        .decide_issue(&service("delegate"), child, NOW)
        .unwrap()
        .is_some());

    let expanded = delegated_grant(
        "expanded",
        worker("worker").id().clone(),
        [AuthorizationAction::CancelCeremony],
        DelegationDepth::none(),
        service("delegate"),
        AuthorizationGrantId::new("parent").unwrap(),
        Some(parent_until + Duration::seconds(1)),
    );
    assert!(policy
        .decide_issue(&service("delegate"), expanded, NOW)
        .is_err());
}

#[test]
fn separation_requires_a_live_approval_from_another_principal_for_the_same_target() {
    let rule = SeparationRule::new(
        AuthorizationAction::ApproveCeremonyGuard,
        AuthorizationAction::CompleteCeremonyStep,
    )
    .unwrap();
    let mut policy = opened_policy(vec![rule]);
    for grant in [
        grant(
            "approver",
            human("approver").id().clone(),
            [AuthorizationAction::ApproveCeremonyGuard],
            DelegationDepth::none(),
            trusted_host(),
            None,
        ),
        grant(
            "executor",
            worker("executor").id().clone(),
            [AuthorizationAction::CompleteCeremonyStep],
            DelegationDepth::none(),
            trusted_host(),
            None,
        ),
    ] {
        issue(&mut policy, &trusted_host(), grant);
    }

    let approval = request(
        "approval",
        human("approver"),
        AuthorizationAction::ApproveCeremonyGuard,
        b"step-result",
    );
    let (approval, event) = policy
        .decide_authorize(approval, NOW, ttl())
        .unwrap()
        .into_parts();
    apply(&mut policy, event.unwrap());

    let execution = request(
        "execution",
        worker("executor"),
        AuthorizationAction::CompleteCeremonyStep,
        b"step-result",
    )
    .with_approval(approval.id().clone());
    assert_eq!(
        policy
            .decide_authorize(execution, NOW, ttl())
            .unwrap()
            .decision()
            .kind(),
        AuthorizationDecisionKind::Allow
    );

    let wrong_target = request(
        "wrong-target",
        worker("executor"),
        AuthorizationAction::CompleteCeremonyStep,
        b"another-result",
    )
    .with_approval(approval.id().clone());
    assert_eq!(
        policy
            .decide_authorize(wrong_target, NOW, ttl())
            .unwrap()
            .decision()
            .denial_reason(),
        Some(AuthorizationDenialReason::ApprovalInvalid)
    );

    revoke(
        &mut policy,
        &trusted_host(),
        &AuthorizationGrantId::new("approver").unwrap(),
        NOW + Duration::seconds(1),
    );
    let revoked_approval = request(
        "revoked-approval",
        worker("executor"),
        AuthorizationAction::CompleteCeremonyStep,
        b"step-result",
    )
    .with_approval(approval.id().clone());
    assert_eq!(
        policy
            .decide_authorize(revoked_approval, NOW + Duration::seconds(1), ttl())
            .unwrap()
            .decision()
            .denial_reason(),
        Some(AuthorizationDenialReason::ApprovalInvalid)
    );
}

#[test]
fn separation_does_not_treat_another_kind_with_the_same_principal_id_as_another_person() {
    let rule = SeparationRule::new(
        AuthorizationAction::ApproveCeremonyGuard,
        AuthorizationAction::CompleteCeremonyStep,
    )
    .unwrap();
    let mut policy = opened_policy(vec![rule]);
    issue(
        &mut policy,
        &trusted_host(),
        grant(
            "dual-role",
            human("dual-role").id().clone(),
            [
                AuthorizationAction::ApproveCeremonyGuard,
                AuthorizationAction::CompleteCeremonyStep,
            ],
            DelegationDepth::none(),
            trusted_host(),
            None,
        ),
    );
    let (approval, event) = policy
        .decide_authorize(
            request(
                "same-id-approval",
                human("dual-role"),
                AuthorizationAction::ApproveCeremonyGuard,
                b"step-result",
            ),
            NOW,
            ttl(),
        )
        .unwrap()
        .into_parts();
    apply(&mut policy, event.unwrap());

    let execution = request(
        "same-id-execution",
        worker("dual-role"),
        AuthorizationAction::CompleteCeremonyStep,
        b"step-result",
    )
    .with_approval(approval.id().clone());
    assert_eq!(
        policy
            .decide_authorize(execution, NOW, ttl())
            .unwrap()
            .decision()
            .denial_reason(),
        Some(AuthorizationDenialReason::ApprovalInvalid)
    );
}

#[test]
fn expired_grant_is_a_persistable_denial() {
    let mut policy = opened_policy(Vec::new());
    let expired = grant(
        "expired",
        worker("worker").id().clone(),
        [AuthorizationAction::ClaimCeremonyStep],
        DelegationDepth::none(),
        trusted_host(),
        Some(NOW + Duration::seconds(1)),
    );
    issue(&mut policy, &trusted_host(), expired);
    let decision = policy
        .decide_authorize(
            request(
                "late",
                worker("worker"),
                AuthorizationAction::ClaimCeremonyStep,
                b"claim",
            ),
            NOW + Duration::seconds(1),
            ttl(),
        )
        .unwrap();
    assert_eq!(decision.decision().kind(), AuthorizationDecisionKind::Deny);
}

#[test]
fn malformed_deserialized_grant_is_rejected_without_mutating_policy() {
    let malformed: AuthorizationGrant = serde_json::from_value(serde_json::json!({
        "id": "malformed",
        "grantee": "worker",
        "actions": [],
        "scope": { "kind": "global" },
        "valid_from": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T11:59:59Z",
        "delegation_depth": 0,
        "issuer": {
            "principal": {
                "id": "trusted-host",
                "kind": "trusted_host",
                "method": "local_host_policy"
            }
        }
    }))
    .unwrap();
    assert!(malformed.validate().is_err());

    let mut policy = opened_policy(Vec::new());
    let before = policy.clone();
    let event = AuthorizationPolicyEvent::GrantIssued {
        policy_id: policy_id(),
        grant: malformed,
        issued_at: NOW,
    };
    assert!(policy.apply(event).is_err());
    assert_eq!(policy, before);
}

#[test]
fn invalid_open_event_leaves_empty_policy_unchanged() {
    let rule = SeparationRule::new(
        AuthorizationAction::ApproveCeremonyGuard,
        AuthorizationAction::CompleteCeremonyStep,
    )
    .unwrap();
    let mut policy = AuthorizationPolicy::empty();
    let before = policy.clone();
    let event = AuthorizationPolicyEvent::Opened {
        policy_id: policy_id(),
        owner: trusted_host(),
        separation_rules: vec![rule, rule],
        opened_at: NOW,
    };
    assert!(policy.apply(event).is_err());
    assert_eq!(policy, before);
}

#[test]
fn revoking_parent_grant_blocks_descendant_authority_and_further_delegation() {
    let mut policy = opened_policy(Vec::new());
    let parent = grant(
        "parent",
        service("delegate").id().clone(),
        [
            AuthorizationAction::IssueAuthorizationGrant,
            AuthorizationAction::PauseCeremony,
        ],
        DelegationDepth::new(2).unwrap(),
        trusted_host(),
        Some(NOW + Duration::minutes(10)),
    );
    issue(&mut policy, &trusted_host(), parent);
    let child = delegated_grant(
        "child",
        worker("worker").id().clone(),
        [
            AuthorizationAction::IssueAuthorizationGrant,
            AuthorizationAction::PauseCeremony,
        ],
        DelegationDepth::new(1).unwrap(),
        service("delegate"),
        AuthorizationGrantId::new("parent").unwrap(),
        Some(NOW + Duration::minutes(5)),
    );
    issue(&mut policy, &service("delegate"), child);
    revoke(
        &mut policy,
        &trusted_host(),
        &AuthorizationGrantId::new("parent").unwrap(),
        NOW + Duration::seconds(1),
    );

    let outcome = policy
        .decide_authorize(
            request(
                "descendant-request",
                worker("worker"),
                AuthorizationAction::PauseCeremony,
                b"ceremony",
            ),
            NOW + Duration::seconds(1),
            ttl(),
        )
        .unwrap();
    assert_eq!(outcome.decision().kind(), AuthorizationDecisionKind::Deny);

    let grandchild = delegated_grant(
        "grandchild",
        human("operator").id().clone(),
        [AuthorizationAction::PauseCeremony],
        DelegationDepth::none(),
        worker("worker"),
        AuthorizationGrantId::new("child").unwrap(),
        Some(NOW + Duration::minutes(2)),
    );
    assert!(policy
        .decide_issue(&worker("worker"), grandchild, NOW + Duration::seconds(1))
        .is_err());
}

fn opened_policy(rules: Vec<SeparationRule>) -> AuthorizationPolicy {
    let mut policy = AuthorizationPolicy::empty();
    let event = policy
        .decide_open(policy_id(), trusted_host(), rules, NOW)
        .unwrap()
        .unwrap();
    apply(&mut policy, event);
    policy
}

fn apply(policy: &mut AuthorizationPolicy, event: AuthorizationPolicyEvent) {
    policy.apply(event).unwrap();
}

fn issue(
    policy: &mut AuthorizationPolicy,
    issuer: &AuthenticatedPrincipal,
    grant: AuthorizationGrant,
) {
    let event = policy.decide_issue(issuer, grant, NOW).unwrap().unwrap();
    apply(policy, event);
}

fn revoke(
    policy: &mut AuthorizationPolicy,
    issuer: &AuthenticatedPrincipal,
    grant_id: &AuthorizationGrantId,
    now: OffsetDateTime,
) {
    let event = policy
        .decide_revoke(
            issuer,
            grant_id,
            AuthorizationRevocationReason::new("operator revoked").unwrap(),
            now,
        )
        .unwrap()
        .unwrap();
    apply(policy, event);
}

fn policy_id() -> AuthorizationPolicyId {
    AuthorizationPolicyId::new("local-policy").unwrap()
}

fn trusted_host() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new("trusted-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}

fn worker(id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap()
}

fn human(id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::Human,
        AuthenticationMethod::MutualTls,
    )
    .unwrap()
}

fn service(id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::Service,
        AuthenticationMethod::TrustedTransportMetadata,
    )
    .unwrap()
}

fn grant<const N: usize>(
    id: &str,
    grantee: PrincipalId,
    actions: [AuthorizationAction; N],
    depth: DelegationDepth,
    issued_by: AuthenticatedPrincipal,
    valid_until: Option<OffsetDateTime>,
) -> AuthorizationGrant {
    AuthorizationGrant::new(
        AuthorizationGrantId::new(id).unwrap(),
        grantee,
        actions,
        AuthorizationScope::Global,
        (NOW, valid_until),
        depth,
        AuthorizationGrantIssuer::direct(issued_by),
    )
    .unwrap()
}

fn delegated_grant<const N: usize>(
    id: &str,
    grantee: PrincipalId,
    actions: [AuthorizationAction; N],
    depth: DelegationDepth,
    issued_by: AuthenticatedPrincipal,
    parent: AuthorizationGrantId,
    valid_until: Option<OffsetDateTime>,
) -> AuthorizationGrant {
    AuthorizationGrant::new(
        AuthorizationGrantId::new(id).unwrap(),
        grantee,
        actions,
        AuthorizationScope::Global,
        (NOW, valid_until),
        depth,
        AuthorizationGrantIssuer::delegated(issued_by, parent),
    )
    .unwrap()
}

fn request(
    id: &str,
    principal: AuthenticatedPrincipal,
    action: AuthorizationAction,
    target: &[u8],
) -> AuthorizationRequest {
    AuthorizationRequest::new(
        AuthorizationRequestId::new(id).unwrap(),
        principal,
        action,
        AuthorizationScope::Global,
        AuthorizationTargetDigest::for_bytes(target),
    )
}

fn ttl() -> AuthorizationDecisionTtl {
    AuthorizationDecisionTtl::from_seconds(30).unwrap()
}
