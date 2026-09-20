//! Handing a paused session to a successor, over the store the
//! edition ships with.
//!
//! The whole journey by the operator's own path: publish the version
//! that replaces the one in flight, pause, read the plan, seal the
//! handoff and open the successor, retry the same request, and reopen
//! the store to see that both ceremonies are still there and still
//! point at each other.

use made_adapters::memory::InMemoryAuthorizationPolicyStore;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    TrustedHostAuthorizationGate,
};
use made_app::services::AuthorizationOperationScope;
use made_app::usecases::{
    PlanCeremonySuccessorInput, StartCeremonyInput, StartCeremonyStepInput,
    StartCeremonySuccessorInput,
};
use made_core::ports::ClockPort;
use made_core::value_objects::*;
use made_embedded::EmbeddedMade;
use std::sync::Arc;
use time::OffsetDateTime;

const V1: &str = r#"
version: "1.0"
name: succession_journey
states: [{id: WORK, initial: true}, {id: DONE, terminal: true}]
transitions: [{from: WORK, to: DONE, trigger: finish}]
steps: [{id: work, state: WORK, handler: host_callback}]
roles: [{id: WORKER, allowed_actions: [work, finish]}]
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
"#;

/// The version the handoff moves to: `work` survives, so the diff
/// carries it, and a review stage is added after it.
const V2: &str = r#"
version: "2.0"
name: succession_journey
states: [{id: WORK, initial: true}, {id: REVIEW}, {id: DONE, terminal: true}]
transitions:
  - {from: WORK, to: REVIEW, trigger: finish}
  - {from: REVIEW, to: DONE, trigger: accept}
steps: [{id: work, state: WORK, handler: host_callback}]
roles: [{id: WORKER, allowed_actions: [work, finish, accept]}]
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
"#;

struct Clock;
impl ClockPort for Clock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }
}

fn origin() -> CeremonyId {
    CeremonyId::new("succession-origin").unwrap()
}

fn plan_id() -> IdempotencyKey {
    IdempotencyKey::new("handoff-1").unwrap()
}

fn scratch() -> std::path::PathBuf {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/succession-187");
    std::fs::create_dir_all(&scratch).unwrap();
    scratch
}

fn handoff() -> StartCeremonySuccessorInput {
    StartCeremonySuccessorInput {
        instance_id: origin(),
        plan_id: plan_id(),
        definition_name: CeremonyName::new("succession_journey").unwrap(),
        definition_version: CeremonyVersion::new("2.0").unwrap(),
        carried: vec![StepId::new("work").unwrap()],
        dispositions: Vec::new(),
        budget: BudgetDisposition::Fresh,
        context_overrides: None,
        actor_id: AuditActorId::new("operator"),
        actor_kind: AuditActorKind::Human,
    }
}

async fn open(path: &std::path::Path) -> EmbeddedMade {
    let engine = EmbeddedMade::builder()
        .with_ceremony_store(Arc::new(SqliteCeremonyStore::open(path).unwrap()))
        .with_clock(Arc::new(Clock))
        .build();
    for yaml in [V1, V2] {
        let definition = engine.mount_yaml(yaml).await.unwrap().definitions()[0].clone();
        engine.publish_definition(definition).await.unwrap();
    }
    engine
}

/// A paused origin whose only step is completed, ready to hand off.
async fn paused_origin(engine: &EmbeddedMade) {
    engine
        .start(StartCeremonyInput::new(
            origin(),
            CeremonyName::new("succession_journey").unwrap(),
            CeremonyVersion::v1(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let claim = Box::pin(engine.start_step(StartCeremonyStepInput::new(
        origin(),
        RoleId::new("WORKER").unwrap(),
        AuditActorKind::Agent,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new("host-1").unwrap(),
        IdempotencyKey::new("claim-1").unwrap(),
        DurationMs::from_millis(60_000),
    )))
    .await
    .unwrap();
    Box::pin(
        engine.complete_step(made_app::usecases::CompleteCeremonyStepInput::new(
            origin(),
            StepId::new("work").unwrap(),
            StepResult::completed(StepOutput::new(
                Attributes::new(std::collections::BTreeMap::from([(
                    "decision".to_owned(),
                    serde_json::json!("ship"),
                )]))
                .unwrap(),
            ))
            .unwrap(),
            AuditActorKind::Agent,
            claim.claim_fence().clone(),
        )),
    )
    .await
    .unwrap();
    engine
        .pause_ceremony(made_app::usecases::PauseCeremonyInput::new(
            origin(),
            "operator",
            AuditActorKind::Human,
            LifecycleReason::new("the definition is wrong").unwrap(),
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn the_whole_journey_survives_a_reopen_and_a_retry() {
    let dir = tempfile::tempdir_in(scratch()).unwrap();
    let path = dir.path().join("store.sqlite3");
    let engine = open(&path).await;
    paused_origin(&engine).await;

    let plan = engine
        .plan_successor(PlanCeremonySuccessorInput {
            instance_id: origin(),
            definition_name: CeremonyName::new("succession_journey").unwrap(),
            definition_version: CeremonyVersion::new("2.0").unwrap(),
        })
        .await
        .unwrap();
    assert!(plan.ready, "blockers: {:?}", plan.blockers);
    assert_eq!(plan.proposed_carried.len(), 1);
    assert_eq!(
        plan.proposed_carried[0].step_id,
        StepId::new("work").unwrap()
    );
    assert!(plan.required_dispositions.is_empty());
    assert!(plan.strands.is_empty());

    let outcome = Box::pin(engine.start_successor(handoff())).await.unwrap();
    let successor_id = outcome.successor.id().clone();
    assert!(successor_id.as_str().starts_with("succession-origin.s."));

    // The same request again: nothing sealed twice, nothing opened twice.
    let again = Box::pin(engine.start_successor(handoff())).await.unwrap();
    assert_eq!(again.successor.id(), &successor_id);
    assert_eq!(again.plan, outcome.plan);

    // The origin has named its successor and can no longer resume.
    let refused = engine
        .resume_ceremony(made_app::usecases::ResumeCeremonyInput::new(
            origin(),
            "operator",
            AuditActorKind::Human,
        ))
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("superseded_by_successor"),
        "{refused}"
    );

    drop(engine);
    let reopened = open(&path).await;
    let origin_instance = reopened.instance(&origin()).await.unwrap();
    assert_eq!(
        origin_instance
            .successor_plan()
            .map(SuccessionPlan::plan_id),
        Some(&plan_id())
    );
    let successor = reopened.instance(&successor_id).await.unwrap();
    let succession = successor
        .succession()
        .expect("a successor knows its origin");
    assert_eq!(succession.predecessor_id(), &origin());
    assert_eq!(
        succession.successor_definition().version(),
        &CeremonyVersion::new("2.0").unwrap()
    );
    let carried = successor
        .step_record(&StepId::new("work").unwrap())
        .unwrap();
    assert_eq!(carried.status(), StepStatus::Completed);
    assert_eq!(
        carried.carried_from().map(SourceRecordRef::ceremony_id),
        Some(&origin())
    );
    assert_eq!(
        carried.output().attributes().as_map().get("decision"),
        Some(&serde_json::json!("ship"))
    );
}

fn principal(name: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(name).unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}

/// Ending one ceremony is not the right to open a session under
/// somebody else's definition, so the target is admitted separately.
#[tokio::test]
async fn starting_a_successor_needs_the_target_definition_admitted_too() {
    let dir = tempfile::tempdir_in(scratch()).unwrap();
    let path = dir.path().join("store.sqlite3");
    let engine = open(&path).await;
    paused_origin(&engine).await;

    let clock = Arc::new(Clock);
    let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let policy = AuthorizationPolicyId::new("succession-auth").unwrap();
    let host = principal("host");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy.clone(),
        policies.clone(),
        clock.clone(),
    );
    admin.open(host.clone(), vec![]).await.unwrap();
    admin
        .issue(
            &host,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("ceremony-only").unwrap(),
                host.id().clone(),
                [
                    AuthorizationAction::PlanCeremonySuccessor,
                    AuthorizationAction::StartCeremonySuccessor,
                ],
                AuthorizationScope::Ceremony {
                    ceremony_id: origin(),
                },
                (clock.now(), None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(host.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy.clone(),
        policies.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    let gate = TrustedHostAuthorizationGate::new(authorize, host).unwrap();
    let protected = engine.with_authorization_policy(policy, policies);

    let operation = gate
        .authorize(
            AuthorizationRequestId::new("start-successor").unwrap(),
            AuthorizationAction::StartCeremonySuccessor,
            AuthorizationScope::Ceremony {
                ceremony_id: origin(),
            },
            AuthorizationTargetDigest::for_bytes(b"start-successor"),
            None,
        )
        .await
        .unwrap();
    let refused = Box::pin(AuthorizationOperationScope::run(
        operation,
        protected.start_successor(handoff()),
    ))
    .await
    .unwrap_err();

    assert!(
        refused
            .to_string()
            .contains("does not admit the successor's definition"),
        "{refused}"
    );
}
