#![cfg(feature = "container-postgres")]

use std::sync::Arc;

use made_adapters::postgres::PostgresAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
    AuthorizeOperationUseCase, ReadAuthorizationDecisionsUseCase,
};
use made_core::entities::AuthorizationPolicyEvent;
use made_core::ports::{
    AuthorizationDecisionPage, AuthorizationPolicyAppendOutcome, AuthorizationPolicySnapshot,
    AuthorizationPolicyStorePort, ClockPort,
};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecision,
    AuthorizationDecisionId, AuthorizationDecisionKind, AuthorizationDecisionPageLimit,
    AuthorizationDecisionTtl, AuthorizationDenialReason, AuthorizationGrant, AuthorizationGrantId,
    AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationPolicyVersion,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationRevocationReason,
    AuthorizationScope, AuthorizationTargetDigest, DelegationDepth, PrincipalId, PrincipalKind,
    SeparationRule,
};
use made_tests_integration::postgres_fixture;
use time::{macros::datetime, Duration, OffsetDateTime};

const NOW: OffsetDateTime = datetime!(2026-09-19 12:00:00 UTC);

#[tokio::test]
async fn decisions_reopen_page_and_keep_the_current_projection_constant() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    let store = Arc::new(PostgresAuthorizationPolicyStore::new(pool.clone()));
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id("projection");
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
        .issue(
            &trusted_host(),
            grant(
                "projection-grant",
                &worker(),
                [AuthorizationAction::ClaimCeremonyStep],
            ),
        )
        .await
        .unwrap();
    let initial_bytes: i64 = sqlx::query_scalar(
        "SELECT octet_length(payload)::BIGINT FROM authorization_policy_state WHERE policy_id = $1",
    )
    .bind(policy_id.as_str())
    .fetch_one(&raw)
    .await
    .unwrap();
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let mut first = None;
    for sequence in 0..128 {
        let request = request(
            &format!("projection-request-{sequence:03}"),
            worker(),
            AuthorizationAction::ClaimCeremonyStep,
            format!("target-{sequence}").as_bytes(),
        );
        let decision = authorize
            .execute(request.clone())
            .await
            .unwrap()
            .decision()
            .clone();
        assert_eq!(decision.request(), &request);
        first.get_or_insert((request, decision));
    }
    let projected_bytes: i64 = sqlx::query_scalar(
        "SELECT octet_length(payload)::BIGINT FROM authorization_policy_state WHERE policy_id = $1",
    )
    .bind(policy_id.as_str())
    .fetch_one(&raw)
    .await
    .unwrap();
    assert_eq!(projected_bytes, initial_bytes);

    let reopened = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let (first_request, first_decision) = first.unwrap();
    let retried = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        reopened.clone(),
        Arc::new(FixedClock(NOW)),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    )
    .execute(first_request)
    .await
    .unwrap()
    .decision()
    .clone();
    assert_eq!(retried, first_decision);
    let snapshot = reopened.load(&policy_id).await.unwrap().unwrap();
    assert_eq!(snapshot.policy.owner(), Some(&trusted_host()));

    let reader = ReadAuthorizationDecisionsUseCase::new(policy_id, reopened);
    let first_page = reader
        .execute(None, AuthorizationDecisionPageLimit::new(100).unwrap())
        .await
        .unwrap();
    assert_eq!(first_page.decisions().len(), 100);
    let after = first_page.decisions().last().unwrap().id().clone();
    let second_page = reader
        .execute(
            Some(&after),
            AuthorizationDecisionPageLimit::new(100).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_page.decisions().len(), 28);
}

#[tokio::test]
async fn replicas_share_one_policy_cas_for_authorize_and_revoke() {
    let (pool, _container) = postgres_fixture::start().await;
    let left = Arc::new(PostgresAuthorizationPolicyStore::new(pool.clone()));
    let right = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id("race");
    let grant = grant(
        "race-grant",
        &worker(),
        [AuthorizationAction::ClaimCeremonyStep],
    );
    let grant_id = grant.id().clone();
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        left.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin.issue(&trusted_host(), grant).await.unwrap();
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
    let shared = request(
        "shared-request",
        worker(),
        AuthorizationAction::ClaimCeremonyStep,
        b"same-target",
    );
    let (left_result, right_result) = tokio::join!(
        left_authorize.execute(shared.clone()),
        right_authorize.execute(shared)
    );
    assert_eq!(
        left_result.unwrap().decision(),
        right_result.unwrap().decision()
    );

    let raced = request(
        "race-request",
        worker(),
        AuthorizationAction::ClaimCeremonyStep,
        b"raced-target",
    );
    let revoker = trusted_host();
    let (revocation_result, decided) = tokio::join!(
        admin.revoke(
            &revoker,
            &grant_id,
            AuthorizationRevocationReason::new("race").unwrap()
        ),
        left_authorize.execute(raced.clone())
    );
    revocation_result.unwrap();
    let decided = decided.unwrap().decision().clone();
    assert!(matches!(
        decided.kind(),
        AuthorizationDecisionKind::Allow | AuthorizationDecisionKind::Deny
    ));
    let repeated = left_authorize
        .execute(raced)
        .await
        .unwrap()
        .decision()
        .clone();
    assert_eq!(repeated, decided);
    let after_revoke = left_authorize
        .execute(request(
            "after-revoke",
            worker(),
            AuthorizationAction::ClaimCeremonyStep,
            b"new-target",
        ))
        .await
        .unwrap()
        .decision()
        .clone();
    assert_eq!(after_revoke.kind(), AuthorizationDecisionKind::Deny);
    assert_eq!(
        after_revoke.denial_reason(),
        Some(AuthorizationDenialReason::NoMatchingGrant)
    );
    assert_eq!(
        left.decisions(
            &policy_id,
            None,
            AuthorizationDecisionPageLimit::new(100).unwrap()
        )
        .await
        .unwrap()
        .decisions()
        .len(),
        3
    );
}

/// Deterministic form of #233 on Postgres: the rival records its decision after
/// this replica looked the request up and before it loaded the snapshot, so
/// this replica's append passes the version CAS and collides on the unique
/// index on (policy_id, request_id). The store must report a conflict and the
/// use case must re-read by request_id and return the rival's decision.
#[tokio::test]
async fn request_index_collision_is_a_conflict_resolved_by_rereading_the_decision() {
    let (pool, _container) = postgres_fixture::start().await;
    let left = Arc::new(PostgresAuthorizationPolicyStore::new(pool.clone()));
    let right = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id("collision");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        left.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(
            &trusted_host(),
            grant(
                "collision-grant",
                &worker(),
                [AuthorizationAction::ClaimCeremonyStep],
            ),
        )
        .await
        .unwrap();
    let shared = request(
        "collided-request",
        worker(),
        AuthorizationAction::ClaimCeremonyStep,
        b"same-target",
    );
    let rival_decision = Arc::new(std::sync::OnceLock::new());
    let rival = {
        let authorize = AuthorizeOperationUseCase::new(
            policy_id.clone(),
            left.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(30).unwrap(),
        );
        let shared = shared.clone();
        let recorded = rival_decision.clone();
        Box::pin(async move {
            let decision = authorize.execute(shared).await.unwrap().decision().clone();
            recorded.set(decision).unwrap();
        })
    };
    let racing = Arc::new(RivalWinsBeforeLoad::new(right, rival));
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        racing.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );

    let decided = authorize.execute(shared).await.unwrap().decision().clone();

    assert_eq!(
        racing.append_conflicts(),
        1,
        "the colliding append must surface as exactly one conflict"
    );
    assert_eq!(Some(&decided), rival_decision.get());
    assert_eq!(
        left.decisions(
            &policy_id,
            None,
            AuthorizationDecisionPageLimit::new(100).unwrap()
        )
        .await
        .unwrap()
        .decisions()
        .len(),
        1
    );
}

/// A backend failure while projecting a decision names the phase and the
/// SQLSTATE, table and constraint instead of the opaque
/// "ceremony persistence backend failed", and carries no stored values.
#[tokio::test]
async fn backend_failures_name_the_phase_and_cause_without_stored_values() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    let store = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id("opaque");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin
        .issue(
            &trusted_host(),
            grant(
                "opaque-grant",
                &worker(),
                [AuthorizationAction::ClaimCeremonyStep],
            ),
        )
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE authorization_decisions \
         ADD CONSTRAINT reject_new_decisions CHECK (false) NOT VALID",
    )
    .execute(&raw)
    .await
    .unwrap();
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );

    let error = authorize
        .execute(request(
            "secret-request-id",
            worker(),
            AuthorizationAction::ClaimCeremonyStep,
            b"secret-target",
        ))
        .await
        .unwrap_err();

    let reason = error.to_string();
    assert_eq!(
        reason,
        "postgres authorization store: project authorization decision failed: \
         database error SQLSTATE 23514 on table authorization_decisions \
         (constraint reject_new_decisions)"
    );
    assert!(!reason.contains("secret-request-id"), "{reason}");
    assert!(!reason.contains("ceremony persistence backend"), "{reason}");
    assert!(
        store
            .decision_for_request(
                &policy_id,
                &AuthorizationRequestId::new("secret-request-id").unwrap()
            )
            .await
            .unwrap()
            .is_none(),
        "the failed append must roll back"
    );
}

#[tokio::test]
async fn revoked_approval_is_rejected_after_postgres_reopen() {
    let (pool, _container) = postgres_fixture::start().await;
    let store = Arc::new(PostgresAuthorizationPolicyStore::new(pool.clone()));
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = policy_id("approval");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin
        .open(
            trusted_host(),
            vec![SeparationRule::new(
                AuthorizationAction::ApproveCeremonyGuard,
                AuthorizationAction::CompleteCeremonyStep,
            )
            .unwrap()],
        )
        .await
        .unwrap();
    let approver_grant = grant(
        "approver-grant",
        &approver(),
        [AuthorizationAction::ApproveCeremonyGuard],
    );
    let approver_grant_id = approver_grant.id().clone();
    admin.issue(&trusted_host(), approver_grant).await.unwrap();
    admin
        .issue(
            &trusted_host(),
            grant(
                "executor-grant",
                &worker(),
                [AuthorizationAction::CompleteCeremonyStep],
            ),
        )
        .await
        .unwrap();
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );
    let approval = authorize
        .execute(request(
            "approval-request",
            approver(),
            AuthorizationAction::ApproveCeremonyGuard,
            b"same-step",
        ))
        .await
        .unwrap()
        .decision()
        .clone();
    admin
        .revoke(
            &trusted_host(),
            &approver_grant_id,
            AuthorizationRevocationReason::new("approval revoked").unwrap(),
        )
        .await
        .unwrap();
    let reopened = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let outcome = AuthorizeOperationUseCase::new(
        policy_id,
        reopened,
        Arc::new(FixedClock(NOW + Duration::seconds(1))),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    )
    .execute(
        request(
            "execution-after-revoke",
            worker(),
            AuthorizationAction::CompleteCeremonyStep,
            b"same-step",
        )
        .with_approval(approval.id().clone()),
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.decision().denial_reason(),
        Some(AuthorizationDenialReason::ApprovalInvalid)
    );
}

#[tokio::test]
async fn corrupted_projection_keys_and_journal_gaps_fail_closed() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let raw = sqlx::PgPool::connect(&url).await.unwrap();
    let store = Arc::new(PostgresAuthorizationPolicyStore::new(pool));
    let clock = Arc::new(FixedClock(NOW));
    let tamper_id = policy_id("tamper");
    let tamper_admin = AuthorizationPolicyAdministrationService::new(
        tamper_id.clone(),
        store.clone(),
        clock.clone(),
    );
    tamper_admin.open(trusted_host(), Vec::new()).await.unwrap();
    tamper_admin
        .issue(
            &trusted_host(),
            grant(
                "tamper-grant",
                &worker(),
                [AuthorizationAction::ClaimCeremonyStep],
            ),
        )
        .await
        .unwrap();
    let tampered_request = request(
        "tamper-request",
        worker(),
        AuthorizationAction::ClaimCeremonyStep,
        b"target",
    );
    AuthorizeOperationUseCase::new(
        tamper_id.clone(),
        store.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    )
    .execute(tampered_request.clone())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE authorization_decisions SET decision_id = 'forged' \
         WHERE policy_id = $1 AND request_id = $2",
    )
    .bind(tamper_id.as_str())
    .bind(tampered_request.id().as_str())
    .execute(&raw)
    .await
    .unwrap();
    assert!(store
        .decision_for_request(&tamper_id, tampered_request.id())
        .await
        .is_err());

    let gap_id = policy_id("gap");
    let gap_admin =
        AuthorizationPolicyAdministrationService::new(gap_id.clone(), store.clone(), clock);
    gap_admin.open(trusted_host(), Vec::new()).await.unwrap();
    gap_admin
        .issue(
            &trusted_host(),
            grant(
                "gap-grant",
                &worker(),
                [AuthorizationAction::ClaimCeremonyStep],
            ),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM authorization_policy_state WHERE policy_id = $1")
        .bind(gap_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("DELETE FROM authorization_policy_events WHERE policy_id = $1 AND version = 1")
        .bind(gap_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
    assert!(store.load(&gap_id).await.is_err());
}

#[derive(Debug, Clone, Copy)]
struct FixedClock(OffsetDateTime);

impl ClockPort for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

fn policy_id(suffix: &str) -> AuthorizationPolicyId {
    AuthorizationPolicyId::new(format!("postgres-{suffix}")).unwrap()
}

fn trusted_host() -> AuthenticatedPrincipal {
    principal(
        "trusted-host",
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
}

fn worker() -> AuthenticatedPrincipal {
    principal(
        "worker",
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
}

fn approver() -> AuthenticatedPrincipal {
    principal(
        "approver",
        PrincipalKind::Human,
        AuthenticationMethod::MutualTls,
    )
}

fn principal(
    id: &str,
    kind: PrincipalKind,
    method: AuthenticationMethod,
) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(PrincipalId::new(id).unwrap(), kind, method).unwrap()
}

fn grant<const N: usize>(
    id: &str,
    grantee: &AuthenticatedPrincipal,
    actions: [AuthorizationAction; N],
) -> AuthorizationGrant {
    AuthorizationGrant::new(
        AuthorizationGrantId::new(id).unwrap(),
        grantee.id().clone(),
        actions,
        AuthorizationScope::Global,
        (NOW - Duration::seconds(1), Some(NOW + Duration::minutes(5))),
        DelegationDepth::none(),
        AuthorizationGrantIssuer::direct(trusted_host()),
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

type RivalWin = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

/// Store decorator that lets a rival host finish authorizing the same request
/// between this host's `decision_for_request` lookup and its snapshot `load`,
/// which is exactly the window #233 described. The rival's decision is then
/// recorded at a newer policy version, so this host's version CAS passes and
/// its own decision collides on the `(policy_id, request_id)` unique index.
struct RivalWinsBeforeLoad<S> {
    inner: Arc<S>,
    rival: tokio::sync::Mutex<Option<RivalWin>>,
    append_conflicts: std::sync::atomic::AtomicUsize,
}

impl<S> RivalWinsBeforeLoad<S> {
    fn new(inner: Arc<S>, rival: RivalWin) -> Self {
        Self {
            inner,
            rival: tokio::sync::Mutex::new(Some(rival)),
            append_conflicts: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn append_conflicts(&self) -> usize {
        self.append_conflicts
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl<S: AuthorizationPolicyStorePort> AuthorizationPolicyStorePort for RivalWinsBeforeLoad<S> {
    async fn load(
        &self,
        policy_id: &AuthorizationPolicyId,
    ) -> Result<Option<AuthorizationPolicySnapshot>, made_core::DomainError> {
        let rival = self.rival.lock().await.take();
        if let Some(rival) = rival {
            rival.await;
        }
        self.inner.load(policy_id).await
    }

    async fn append(
        &self,
        policy_id: &AuthorizationPolicyId,
        expected: AuthorizationPolicyVersion,
        events: Vec<AuthorizationPolicyEvent>,
    ) -> Result<AuthorizationPolicyAppendOutcome, made_core::DomainError> {
        let outcome = self.inner.append(policy_id, expected, events).await?;
        if matches!(outcome, AuthorizationPolicyAppendOutcome::Conflict { .. }) {
            self.append_conflicts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(outcome)
    }

    async fn decisions(
        &self,
        policy_id: &AuthorizationPolicyId,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, made_core::DomainError> {
        self.inner.decisions(policy_id, after, limit).await
    }

    async fn decision(
        &self,
        policy_id: &AuthorizationPolicyId,
        decision_id: &AuthorizationDecisionId,
    ) -> Result<Option<AuthorizationDecision>, made_core::DomainError> {
        self.inner.decision(policy_id, decision_id).await
    }

    async fn decision_for_request(
        &self,
        policy_id: &AuthorizationPolicyId,
        request_id: &AuthorizationRequestId,
    ) -> Result<Option<AuthorizationDecision>, made_core::DomainError> {
        self.inner.decision_for_request(policy_id, request_id).await
    }
}
