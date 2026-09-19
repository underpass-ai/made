//! Protected handlers outlive admission TTL; revocation stops the next claim.
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use made_adapters::memory::{InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    TrustedHostAuthorizationGate,
};
use made_app::services::AuthorizationOperationScope;
use made_app::usecases::{RunCeremonyInput, RunCeremonyStepInput, StartCeremonyInput};
use made_core::entities::{AuditChain, AuditRecord, CeremonyDefinition};
use made_core::ports::{AuthorizationPolicyStorePort, CeremonyEventStorePort, ClockPort};
use made_core::value_objects::*;
use made_embedded::EmbeddedMade;
use time::OffsetDateTime;

const DEFINITION: &str = r#"
version: "1.0"
name: long_authorized
states:
  - {id: WORK, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: WORK, to: DONE, trigger: finish}
steps:
  - {id: first, state: WORK, handler: host_callback}
  - {id: second, state: WORK, handler: host_callback}
roles:
  - {id: WORKER, allowed_actions: [first, second, finish]}
"#;

#[derive(Default)]
struct Clock(AtomicI64);
impl ClockPort for Clock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_790_000_000 + self.0.load(Ordering::SeqCst)).unwrap()
    }
}

struct Fixture {
    engine: EmbeddedMade,
    definition: CeremonyDefinition,
    events: Arc<InMemoryCeremonyEventStore>,
    policies: Arc<InMemoryAuthorizationPolicyStore>,
    gate: TrustedHostAuthorizationGate,
    policy_id: AuthorizationPolicyId,
}

impl Fixture {
    async fn new(seconds: i64, revoke: bool) -> Self {
        let clock = Arc::new(Clock::default());
        let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
        let policy_id = AuthorizationPolicyId::new("long-running").unwrap();
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new("host").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        let admin = Arc::new(AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            policies.clone(),
            clock.clone(),
        ));
        admin.open(principal.clone(), vec![]).await.unwrap();
        let grant_id = AuthorizationGrantId::new("run-grant").unwrap();
        admin
            .issue(
                &principal,
                AuthorizationGrant::new(
                    grant_id.clone(),
                    principal.id().clone(),
                    [
                        AuthorizationAction::RunCeremony,
                        AuthorizationAction::RunCeremonyStep,
                        AuthorizationAction::StartCeremony,
                    ],
                    AuthorizationScope::Global,
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(principal.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            policy_id.clone(),
            policies.clone(),
            clock.clone(),
            AuthorizationDecisionTtl::from_seconds(60).unwrap(),
        ));
        let gate = TrustedHostAuthorizationGate::new(authorize, principal.clone()).unwrap();
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        let engine = EmbeddedMade::builder()
            .with_ceremony_store(events.clone())
            .with_clock(clock.clone())
            .with_step_handler_callback({
                let clock = clock.clone();
                move |_| {
                    let clock = clock.clone();
                    let admin = admin.clone();
                    let principal = principal.clone();
                    let grant_id = grant_id.clone();
                    async move {
                        clock.0.fetch_add(seconds, Ordering::SeqCst);
                        if revoke {
                            admin
                                .revoke(
                                    &principal,
                                    &grant_id,
                                    AuthorizationRevocationReason::new(
                                        "operator revoked during work",
                                    )
                                    .unwrap(),
                                )
                                .await
                                .unwrap();
                        }
                        StepResult::completed(StepOutput::empty())
                    }
                }
            })
            .build();
        let definition = engine.mount_yaml(DEFINITION).await.unwrap().definitions()[0].clone();
        let engine = engine.with_authorization_policy(policy_id.clone(), policies.clone());
        Self {
            engine,
            definition,
            events,
            policies,
            gate,
            policy_id,
        }
    }

    async fn operation(&self, action: AuthorizationAction, request: &str) -> AuthorizedOperation {
        self.gate
            .authorize(
                AuthorizationRequestId::new(request).unwrap(),
                action,
                AuthorizationScope::Ceremony { ceremony_id: id() },
                AuthorizationTargetDigest::for_bytes(request.as_bytes()),
                None,
            )
            .await
            .unwrap()
    }

    async fn records(&self) -> Vec<AuditRecord> {
        self.events
            .read(&id(), StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
            .await
            .unwrap()
    }

    async fn assert_evidence(&self, records: &[AuditRecord]) {
        assert!(AuditChain::verify(records).is_intact());
        for record in records {
            let evidence = record.authorization_evidence().unwrap();
            let decision = self
                .policies
                .decision(&self.policy_id, evidence.decision_id())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(decision.evidence(record.occurred_at()).unwrap(), *evidence);
        }
        let completions = records
            .iter()
            .filter(|r| r.event_type() == AuditEventType::StepCompleted);
        for record in completions {
            let evidence = record.authorization_evidence().unwrap();
            let decision = self
                .policies
                .decision(&self.policy_id, evidence.decision_id())
                .await
                .unwrap()
                .unwrap();
            assert!(decision.request().accepted_work_decision_id().is_some());
        }
    }

    fn run_input(&self) -> RunCeremonyInput {
        RunCeremonyInput::new(
            id(),
            self.definition.clone(),
            CeremonyContext::empty(),
            LeaseOwnerId::new("worker").unwrap(),
            DurationMs::from_millis(300_000),
            "operator",
            AuditActorKind::Service,
        )
    }
}

fn id() -> CeremonyId {
    CeremonyId::new("long-running").unwrap()
}

#[tokio::test]
async fn run_ceremony_finishes_multiple_handlers_beyond_admission_ttl() {
    let f = Fixture::new(65, false).await;
    let operation = f.operation(AuthorizationAction::RunCeremony, "run").await;
    let output = AuthorizationOperationScope::run(operation, f.engine.run(f.run_input()))
        .await
        .unwrap();
    assert!(output.instance().is_completed(&f.definition));
    let records = f.records().await;
    assert_eq!(
        records
            .iter()
            .filter(|r| r.event_type() == AuditEventType::StepCompleted)
            .count(),
        2
    );
    f.assert_evidence(&records).await;
}

#[tokio::test]
async fn revocation_drains_the_current_handler_and_prevents_another_claim() {
    for seconds in [5, 65] {
        let f = Fixture::new(seconds, true).await;
        let operation = f.operation(AuthorizationAction::RunCeremony, "run").await;
        let error = AuthorizationOperationScope::run(operation, f.engine.run(f.run_input()))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("authorization"), "{error}");
        let records = f.records().await;
        assert!(AuditChain::verify(&records).is_intact());
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event_type() == AuditEventType::StepStarted)
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event_type() == AuditEventType::StepCompleted)
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn run_step_drains_after_ttl_and_revocation_with_a_sealed_antecedent() {
    let f = Fixture::new(65, true).await;
    let start = f
        .operation(AuthorizationAction::StartCeremony, "start")
        .await;
    AuthorizationOperationScope::run(
        start,
        f.engine.start(StartCeremonyInput::new(
            id(),
            f.definition.name().clone(),
            f.definition.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        )),
    )
    .await
    .unwrap();
    let operation = f
        .operation(AuthorizationAction::RunCeremonyStep, "step")
        .await;
    let output = AuthorizationOperationScope::run(
        operation,
        f.engine.run_step(RunCeremonyStepInput::new(
            id(),
            RoleId::new("WORKER").unwrap(),
            AuditActorKind::Agent,
            StepId::new("first").unwrap(),
            LeaseOwnerId::new("worker").unwrap(),
            IdempotencyKey::new("first").unwrap(),
            DurationMs::from_millis(300_000),
        )),
    )
    .await
    .unwrap();
    assert!(output.result().is_success());
    f.assert_evidence(&f.records().await).await;
}
