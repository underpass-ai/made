#![cfg(feature = "sqlite")]

use std::sync::Arc;
use std::sync::RwLock;

use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
    AuthorizeOperationUseCase, ReadAuthorizationDecisionsUseCase,
};
use made_core::ports::{AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionKind,
    AuthorizationDecisionPageLimit, AuthorizationDecisionTtl, AuthorizationDenialReason,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationRevocationReason,
    AuthorizationScope, AuthorizationTargetDigest, DelegationDepth, PrincipalId, PrincipalKind,
};
use tempfile::TempDir;
use time::{macros::datetime, Duration, OffsetDateTime};

const NOW: OffsetDateTime = datetime!(2026-09-19 12:00:00 UTC);

#[tokio::test]
async fn decisions_survive_reopen_and_exact_request_retries() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id();
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    assert!(matches!(
        admin.open(trusted_host(), Vec::new()).await.unwrap(),
        AuthorizationMutationOutcome::Applied { .. }
    ));
    admin
        .issue(&trusted_host(), worker_grant("grant-1"))
        .await
        .unwrap();
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let request = claim_request("request-1", b"claim");
    let first = authorize
        .execute(request.clone())
        .await
        .unwrap()
        .decision()
        .clone();

    let reopened = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        reopened.clone(),
        Arc::new(FixedClock(NOW)),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let repeated = authorize.execute(request).await.unwrap().decision().clone();
    assert_eq!(repeated, first);

    let reader = ReadAuthorizationDecisionsUseCase::new(policy_id, reopened);
    let page = reader
        .execute(None, AuthorizationDecisionPageLimit::new(10).unwrap())
        .await
        .unwrap();
    assert_eq!(page.decisions(), &[first]);
}

#[tokio::test]
async fn two_hosts_order_revoke_and_authorize_in_one_policy_cas() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-race.db");
    let left = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let right = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id();
    let owner = trusted_host();
    let grant = worker_grant("race-grant");
    let grant_id = grant.id().clone();
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        left.clone(),
        clock.clone(),
    );
    admin.open(owner.clone(), Vec::new()).await.unwrap();
    admin.issue(&owner, grant).await.unwrap();

    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        right,
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let request = claim_request("race-request", b"race-claim");
    let revoke = admin.revoke(
        &owner,
        &grant_id,
        AuthorizationRevocationReason::new("race").unwrap(),
    );
    let decide = authorize.execute(request.clone());
    let (revoked, decision) = tokio::join!(revoke, decide);
    revoked.unwrap();
    let decision = decision.unwrap().decision().clone();
    assert!(matches!(
        decision.kind(),
        AuthorizationDecisionKind::Allow | AuthorizationDecisionKind::Deny
    ));
    if decision.kind() == AuthorizationDecisionKind::Deny {
        assert_eq!(
            decision.denial_reason(),
            Some(AuthorizationDenialReason::NoMatchingGrant)
        );
    }

    let repeated = authorize.execute(request).await.unwrap().decision().clone();
    assert_eq!(repeated, decision);
    let snapshot = left.load(&policy_id).await.unwrap().unwrap();
    assert_eq!(snapshot.policy.decisions().count(), 1);
}

#[tokio::test]
async fn concurrent_hosts_record_one_decision_for_the_same_request() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-idempotency.db");
    let left = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let right = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let policy_id = policy_id();
    let clock = Arc::new(FixedClock(NOW));
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        left.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(&trusted_host(), worker_grant("shared-grant"))
        .await
        .unwrap();
    let left_authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        left.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let right_authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        right,
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let request = claim_request("shared-request", b"same-target");
    let (left_result, right_result) = tokio::join!(
        left_authorize.execute(request.clone()),
        right_authorize.execute(request)
    );
    assert_eq!(
        left_result.unwrap().decision(),
        right_result.unwrap().decision()
    );
    assert_eq!(
        left.load(&policy_id)
            .await
            .unwrap()
            .unwrap()
            .policy
            .decisions()
            .count(),
        1
    );
}

#[tokio::test]
async fn expired_historical_allow_cannot_be_used_as_fresh_admission() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-expiry.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let clock = Arc::new(MutableClock::new(NOW));
    let policy_id = policy_id();
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(&trusted_host(), worker_grant("expiry-grant"))
        .await
        .unwrap();
    let authorize = AuthorizeOperationUseCase::new(
        policy_id,
        store,
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(1).unwrap(),
    );
    let request = claim_request("expiry-request", b"expiry-target");
    assert!(authorize
        .execute(request.clone())
        .await
        .unwrap()
        .evidence()
        .is_some());
    clock.set(NOW + Duration::seconds(2));
    let expired = authorize.execute(request).await.unwrap();
    assert!(expired.evidence().is_none());
    assert!(matches!(
        expired,
        made_app::authorization::AuthorizationGateOutcome::Expired { .. }
    ));
}

#[derive(Debug, Clone, Copy)]
struct FixedClock(OffsetDateTime);

impl ClockPort for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

#[derive(Debug)]
struct MutableClock(RwLock<OffsetDateTime>);

impl MutableClock {
    fn new(now: OffsetDateTime) -> Self {
        Self(RwLock::new(now))
    }

    fn set(&self, now: OffsetDateTime) {
        *self.0.write().unwrap() = now;
    }
}

impl ClockPort for MutableClock {
    fn now(&self) -> OffsetDateTime {
        *self.0.read().unwrap()
    }
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

fn worker() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new("worker").unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap()
}

fn worker_grant(id: &str) -> AuthorizationGrant {
    AuthorizationGrant::new(
        AuthorizationGrantId::new(id).unwrap(),
        worker().id().clone(),
        [AuthorizationAction::ClaimCeremonyStep],
        AuthorizationScope::Global,
        (NOW - Duration::seconds(1), Some(NOW + Duration::minutes(5))),
        DelegationDepth::none(),
        AuthorizationGrantIssuer::direct(trusted_host().id().clone()),
    )
    .unwrap()
}

fn claim_request(id: &str, target: &[u8]) -> AuthorizationRequest {
    AuthorizationRequest::new(
        AuthorizationRequestId::new(id).unwrap(),
        worker(),
        AuthorizationAction::ClaimCeremonyStep,
        AuthorizationScope::Global,
        AuthorizationTargetDigest::for_bytes(target),
    )
}
