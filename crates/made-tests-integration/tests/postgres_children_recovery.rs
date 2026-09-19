//! Restart recovery for child plans, terminal children and joins without NATS.

#![cfg(feature = "container-tests")]

use std::sync::Arc;

use made_adapters::postgres::{PostgresCeremonyStore, PostgresConfig, PostgresPool};
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{ApplyCeremonyTransitionInput, RunCeremonyStepInput, StartCeremonyInput};
use made_core::entities::{CeremonyDefinition, CeremonyEvent};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyEventPageLimit, CeremonyId, DurationMs,
    IdempotencyKey, LeaseOwnerId, RoleId, StepId, TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use made_tests_integration::parity_clock::ParityClock;
use made_tests_integration::parity_step_handler::ParityStepHandler;
use made_tests_integration::postgres_fixture;
use tokio::process::Command;

const MODE: &str = "MADE_CHILD_RECOVERY_HELPER_MODE";
const URL: &str = "MADE_CHILD_RECOVERY_POSTGRES_URL";

const CHILD_YAML: &str = r#"
version: "1.0"
name: postgres_recovery_child
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [work_done]
steps:
  - id: work
    state: OPEN
    handler: parity_step
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: CHILD
    allowed_actions: [work, finish]
"#;

const PARENT_YAML: &str = r#"
version: "1.0"
name: postgres_recovery_parent
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [child_done]
steps:
  - id: delegate
    state: OPEN
    handler: must_not_run
    spawn:
      children:
        - ceremony: postgres_recovery_child
          version: "1.0"
          inputs: {}
      max_children: 1
      max_depth: 2
guards:
  child_done:
    type: automated
    check: "children_completed:delegate:all"
roles:
  - id: PARENT
    allowed_actions: [delegate, finish]
"#;

fn definition(raw: &str) -> CeremonyDefinition {
    CeremonyDefinitionYaml::parse_str(raw).unwrap()
}

fn engine(store: Arc<PostgresCeremonyStore>) -> EmbeddedMade {
    EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_definition_publications(store.clone())
        .with_event_cursor(store)
        .with_clock(ParityClock::shared())
        .with_step_handler(ParityStepHandler::shared())
        .build()
}

fn step(ceremony_id: &CeremonyId, step_id: &str, role_id: &str) -> RunCeremonyStepInput {
    RunCeremonyStepInput::new(
        ceremony_id.clone(),
        RoleId::new(role_id).unwrap(),
        AuditActorKind::Agent,
        StepId::new(step_id).unwrap(),
        LeaseOwnerId::new("postgres-recovery").unwrap(),
        IdempotencyKey::new(format!("{ceremony_id}-{step_id}")).unwrap(),
        DurationMs::from_millis(30_000),
    )
}

fn transition(ceremony_id: &CeremonyId, role_id: &str) -> ApplyCeremonyTransitionInput {
    ApplyCeremonyTransitionInput::new(
        ceremony_id.clone(),
        RoleId::new(role_id).unwrap(),
        AuditActorKind::Agent,
        TransitionTrigger::new("finish").unwrap(),
    )
}

async fn leave_terminal_child_unaccepted(
    engine: &EmbeddedMade,
    parent_id: &CeremonyId,
) -> CeremonyId {
    let parent = definition(PARENT_YAML);
    engine
        .start_published(StartCeremonyInput::new(
            parent_id.clone(),
            parent.name().clone(),
            parent.version().clone(),
            CeremonyContext::empty(),
            "postgres-recovery",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    Box::pin(engine.run_step(step(parent_id, "delegate", "PARENT")))
        .await
        .unwrap();
    let child_id = engine
        .instance(parent_id)
        .await
        .unwrap()
        .child_groups()
        .values()
        .next()
        .unwrap()
        .plan()
        .children()[0]
        .child_id()
        .clone();
    Box::pin(engine.run_step(step(&child_id, "work", "CHILD")))
        .await
        .unwrap();
    engine
        .apply_transition(transition(&child_id, "CHILD"))
        .await
        .unwrap();
    assert!(engine
        .audit_records(&child_id)
        .await
        .unwrap()
        .iter()
        .any(|record| matches!(record.event(), Some(CeremonyEvent::CeremonyCompleted(_)))));
    assert!(engine
        .instance(parent_id)
        .await
        .unwrap()
        .child_groups()
        .values()
        .next()
        .unwrap()
        .completions()
        .is_empty());
    child_id
}

#[tokio::test]
async fn rolling_restart_recovers_child_plan_completion_and_join_without_nats() {
    let (_pool, url, _container) = postgres_fixture::start_with_url().await;
    for mode in ["prepare", "recover", "verify"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("replica_process")
            .arg("--exact")
            .arg("--ignored")
            .arg("--nocapture")
            .env(MODE, mode)
            .env(URL, &url)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{mode} replica failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[tokio::test]
#[ignore = "subprocess entrypoint"]
async fn replica_process() {
    let url = std::env::var(URL).unwrap();
    let pool = PostgresPool::connect(&PostgresConfig::from_url(url))
        .await
        .unwrap();
    let engine = engine(Arc::new(PostgresCeremonyStore::new(pool)));
    match std::env::var(MODE).unwrap().as_str() {
        "prepare" => prepare_replica(&engine).await,
        "recover" => recover_replica(&engine).await,
        "verify" => verify_replica(&engine).await,
        mode => panic!("unknown child recovery helper mode {mode}"),
    }
}

async fn prepare_replica(first: &EmbeddedMade) {
    first
        .publish_definition(definition(CHILD_YAML))
        .await
        .unwrap();
    first
        .publish_definition(definition(PARENT_YAML))
        .await
        .unwrap();
    let parent_id = CeremonyId::new("postgres-recovery-parent-1").unwrap();
    Box::pin(leave_terminal_child_unaccepted(first, &parent_id)).await;
}

async fn recover_replica(restarted: &EmbeddedMade) {
    let round = restarted
        .recover_children(CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert!(round.recovered_plans >= 1);
    assert_eq!(round.accepted_completions, 1);
    assert_eq!(round.failed, 0);
    let parent_id = CeremonyId::new("postgres-recovery-parent-1").unwrap();
    let parent = restarted.instance(&parent_id).await.unwrap();
    let group = parent.child_groups().values().next().unwrap();
    assert_eq!(group.plan().children().len(), 1);
    assert_eq!(group.completions().len(), 1);
    restarted
        .apply_transition(transition(&parent_id, "PARENT"))
        .await
        .unwrap();
    assert!(restarted
        .audit_records(&parent_id)
        .await
        .unwrap()
        .iter()
        .any(|record| matches!(record.event(), Some(CeremonyEvent::CeremonyCompleted(_)))));
}

async fn verify_replica(second_restart: &EmbeddedMade) {
    let replay = second_restart
        .recover_children(CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(replay.accepted_completions, 0);
    assert_eq!(replay.failed, 0);
    let parent_id = CeremonyId::new("postgres-recovery-parent-1").unwrap();
    let parent = second_restart.instance(&parent_id).await.unwrap();
    assert_eq!(
        parent
            .child_groups()
            .values()
            .next()
            .unwrap()
            .completions()
            .len(),
        1
    );
    assert!(second_restart
        .audit_records(&parent_id)
        .await
        .unwrap()
        .iter()
        .any(|record| matches!(record.event(), Some(CeremonyEvent::CeremonyCompleted(_)))));
}
