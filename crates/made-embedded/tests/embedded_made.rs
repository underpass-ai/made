use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use made_adapters::yaml::ProtocolDefinitionYaml;
use made_app::usecases::{
    ApplyProtocolTransitionInput, ApproveProtocolGuardInput, RunProtocolInput, StartProtocolInput,
};
use made_core::entities::ProtocolDefinition;
use made_core::value_objects::{
    AuditActorKind, DurationMs, GuardName, LeaseOwnerId, ProtocolContext, ProtocolId, RoleId,
    StepOutput, StepResult, TransitionTrigger,
};
use made_embedded::{EmbeddedMade, VERSION};

const LINEAR_PROTOCOL: &str = r#"
version: "1.0"
name: "embedded_linear"
states:
  - id: STARTED
    initial: true
  - id: COMPLETED
    terminal: true
transitions:
  - from: STARTED
    to: COMPLETED
    trigger: finish
    guards:
      - work_completed
steps:
  - id: work
    state: STARTED
    handler: host_callback
guards:
  work_completed:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: SYSTEM
    allowed_actions:
      - work
      - finish
"#;

const HUMAN_GUARD_PROTOCOL: &str = r#"
version: "1.0"
name: "embedded_human_guard"
states:
  - id: WAITING
    initial: true
  - id: APPROVED
    terminal: true
transitions:
  - from: WAITING
    to: APPROVED
    trigger: approve
    guards:
      - human_approved
guards:
  human_approved:
    type: human
    check: manual_approval
roles:
  - id: APPROVER
    allowed_actions:
      - approve
"#;

fn input(definition: ProtocolDefinition) -> RunProtocolInput {
    RunProtocolInput::new(
        ProtocolId::new("embedded-run").unwrap(),
        definition,
        ProtocolContext::empty(),
        LeaseOwnerId::new("embedded-host").unwrap(),
        DurationMs::from_millis(30_000),
        "operator-1",
        AuditActorKind::Service,
    )
}

#[tokio::test]
async fn default_runtime_runs_without_external_services() {
    let embedded = EmbeddedMade::default();
    let definition = ProtocolDefinitionYaml::parse_str(LINEAR_PROTOCOL).unwrap();

    let output = Box::pin(embedded.run(input(definition))).await.unwrap();

    assert!(output.instance().is_completed(output.definition()));
    assert_eq!(embedded.version(), VERSION);
    assert_eq!(
        embedded
            .transcript(output.instance().id())
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn host_callback_is_an_embeddable_step_adapter() {
    let called = Arc::new(AtomicBool::new(false));
    let observed = called.clone();
    let embedded = EmbeddedMade::builder()
        .with_step_handler_callback(move |_request| {
            let observed = observed.clone();
            async move {
                observed.store(true, Ordering::SeqCst);
                StepResult::completed(StepOutput::empty())
            }
        })
        .build();
    let definition = ProtocolDefinitionYaml::parse_str(LINEAR_PROTOCOL).unwrap();

    let output = Box::pin(embedded.run(input(definition))).await.unwrap();

    assert!(called.load(Ordering::SeqCst));
    assert!(output.instance().is_completed(output.definition()));
}

#[tokio::test]
async fn mounting_yaml_uses_the_application_mount_use_case() {
    let embedded = EmbeddedMade::default();

    let mounted = embedded.mount_yaml(LINEAR_PROTOCOL).await.unwrap();

    assert_eq!(mounted.definitions().len(), 1);
    assert_eq!(embedded.definitions().await.unwrap().len(), 1);
}

#[tokio::test]
async fn host_can_drive_a_human_guard_incrementally() {
    let embedded = EmbeddedMade::default();
    let mounted = embedded.mount_yaml(HUMAN_GUARD_PROTOCOL).await.unwrap();
    let definition = mounted.definitions()[0].clone();
    let protocol_id = ProtocolId::new("human-active-run").unwrap();

    embedded
        .start(StartProtocolInput::new(
            protocol_id.clone(),
            definition.name().clone(),
            definition.version().clone(),
            ProtocolContext::empty(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    embedded
        .approve_guard(ApproveProtocolGuardInput::new(
            protocol_id.clone(),
            GuardName::new("human_approved").unwrap(),
            RoleId::new("APPROVER").unwrap(),
            AuditActorKind::Human,
        ))
        .await
        .unwrap();
    let transitioned = embedded
        .apply_transition(ApplyProtocolTransitionInput::new(
            protocol_id.clone(),
            RoleId::new("APPROVER").unwrap(),
            AuditActorKind::Agent,
            TransitionTrigger::new("approve").unwrap(),
        ))
        .await
        .unwrap();

    assert!(transitioned.is_completed(&definition));
    assert!(embedded
        .instance(&protocol_id)
        .await
        .unwrap()
        .is_completed(&definition));
}
