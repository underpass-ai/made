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
use rusqlite::{params, Connection};
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
    assert_eq!(decision_count(left.as_ref(), &policy_id).await, 1);
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
    assert_eq!(decision_count(left.as_ref(), &policy_id).await, 1);
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

#[tokio::test]
async fn current_policy_projection_stays_constant_as_decision_history_grows() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-projection.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let policy_id = policy_id();
    let clock = Arc::new(FixedClock(NOW));
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(&trusted_host(), worker_grant("projection-grant"))
        .await
        .unwrap();
    let initial_bytes = projection_bytes(&path);
    let authorize = AuthorizeOperationUseCase::new(
        policy_id,
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    for sequence in 0..128 {
        authorize
            .execute(claim_request(
                &format!("projection-request-{sequence}"),
                format!("target-{sequence}").as_bytes(),
            ))
            .await
            .unwrap();
    }
    assert_eq!(projection_bytes(&path), initial_bytes);
}

#[tokio::test]
async fn tampered_decision_projection_keys_fail_closed() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-tamper.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let policy_id = policy_id();
    let clock = Arc::new(FixedClock(NOW));
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(&trusted_host(), worker_grant("tamper-grant"))
        .await
        .unwrap();
    let request = claim_request("tamper-request", b"target");
    AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    )
    .execute(request.clone())
    .await
    .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE authorization_decisions SET decision_id = 'forged' WHERE policy_id = ?1 AND request_id = ?2",
            params![policy_id.as_str(), request.id().as_str()],
        )
        .unwrap();

    assert!(store
        .decision_for_request(&policy_id, request.id())
        .await
        .is_err());
}

#[tokio::test]
async fn non_contiguous_legacy_journal_fails_closed_during_projection_rebuild() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("authorization-gap.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let policy_id = policy_id();
    let clock = Arc::new(FixedClock(NOW));
    let admin =
        AuthorizationPolicyAdministrationService::new(policy_id.clone(), store.clone(), clock);
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(&trusted_host(), worker_grant("gap-grant"))
        .await
        .unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "DELETE FROM authorization_policy_state WHERE policy_id = ?1",
            [policy_id.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "DELETE FROM authorization_policy_events WHERE policy_id = ?1 AND version = 1",
            [policy_id.as_str()],
        )
        .unwrap();

    assert!(store.load(&policy_id).await.is_err());
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

async fn decision_count(
    store: &SqliteAuthorizationPolicyStore,
    policy_id: &AuthorizationPolicyId,
) -> usize {
    store
        .decisions(
            policy_id,
            None,
            AuthorizationDecisionPageLimit::new(100).unwrap(),
        )
        .await
        .unwrap()
        .decisions()
        .len()
}

fn projection_bytes(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT length(payload) FROM authorization_policy_state",
            [],
            |row| row.get(0),
        )
        .unwrap()
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
        AuthorizationGrantIssuer::direct(trusted_host()),
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
