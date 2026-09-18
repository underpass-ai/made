use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{
    ApplyCeremonyTransitionInput, ApproveCeremonyGuardInput, GenerateCeremonyReportInput,
    RunCeremonyInput,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, GuardName, LeaseOwnerId,
    RoleId, StepId, StepOutput, StepResult, StepStatus, TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

const GROUP_CHAT: &str = include_str!("../../../api/examples/ceremonies/fragments/group_chat.yaml");
const MAKER_CHECKER: &str =
    include_str!("../../../api/examples/ceremonies/fragments/maker_checker.yaml");
const HANDOFF: &str = include_str!("../../../api/examples/ceremonies/fragments/handoff.yaml");
const MAGENTIC: &str = include_str!("../../../api/examples/ceremonies/fragments/magentic.yaml");

#[derive(Debug, Clone, Copy)]
enum Scenario {
    GroupEarly,
    GroupCap,
    MakerSecond,
    MakerCap,
    HandoffResolved,
    HandoffBounce,
    LedgerCompleted,
    LedgerStalled,
}

#[derive(Debug)]
struct ScenarioHandler(Scenario);

#[async_trait]
impl CeremonyStepHandlerPort for ScenarioHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let id = request.step_id().as_str();
        let value = match self.0 {
            Scenario::GroupEarly if id.contains("manage") => {
                json!({"done":true, "instructions":"stop"})
            }
            Scenario::GroupCap if id.contains("manage") => {
                json!({"done":false, "instructions":"continue"})
            }
            Scenario::GroupEarly | Scenario::GroupCap if id.contains("select") => {
                json!({"next_speaker":"OPS"})
            }
            Scenario::MakerSecond if id.ends_with("check_1") => json!({"approved":false}),
            Scenario::MakerSecond if id.ends_with("check_2") => json!({"approved":true}),
            Scenario::MakerCap if id.contains("check") => json!({"approved":false}),
            Scenario::HandoffResolved if id.ends_with("_ops") => {
                json!({"resolved":false, "handoff_to":"SECURITY"})
            }
            Scenario::HandoffResolved if id.ends_with("_security") => {
                json!({"resolved":true, "handoff_to":null})
            }
            Scenario::HandoffBounce if id.ends_with("_ops") => {
                let prior_ops = request
                    .transcript()
                    .contributions()
                    .iter()
                    .filter(|contribution| contribution.step_id().as_str().ends_with("_ops"))
                    .count();
                if prior_ops < 2 {
                    json!({"resolved":false, "handoff_to":"SECURITY"})
                } else {
                    json!({"resolved":false, "handoff_to":"HUMAN"})
                }
            }
            Scenario::HandoffBounce if id.ends_with("_security") => {
                json!({"resolved":false, "handoff_to":"OPS"})
            }
            Scenario::LedgerCompleted | Scenario::LedgerStalled if id.ends_with("_plan") => {
                json!({"ledger":{"tasks":[{"id":"task-a","status":"open"}]}})
            }
            Scenario::LedgerCompleted | Scenario::LedgerStalled if id.contains("_pick_") => {
                json!({"owner":"OPS"})
            }
            Scenario::LedgerCompleted if id.ends_with("_update_1") => {
                json!({"ledger":{"tasks":[{"id":"task-a","status":"done"}]}, "done":false, "stalled":false})
            }
            Scenario::LedgerCompleted if id.ends_with("_update_2") => {
                json!({"ledger":{"tasks":[{"id":"task-a","status":"done"}]}, "done":true, "stalled":false})
            }
            Scenario::LedgerStalled if id.ends_with("_update_1") => {
                json!({"ledger":{"tasks":[{"id":"task-a","status":"blocked"}]}, "done":false, "stalled":true})
            }
            _ => json!({"result": format!("completed {id}")}),
        };
        let Value::Object(fields) = value else {
            unreachable!()
        };
        StepResult::completed(StepOutput::new(Attributes::new(
            fields.into_iter().collect::<BTreeMap<_, _>>(),
        )?))
    }
}

async fn run(id: &str, yaml: &str, scenario: Scenario) -> (EmbeddedMade, CeremonyId) {
    let definition = CeremonyDefinitionYaml::parse_str(yaml).unwrap();
    let engine = EmbeddedMade::builder()
        .with_step_handler(Arc::new(ScenarioHandler(scenario)))
        .build();
    let ceremony_id = CeremonyId::new(id).unwrap();
    let output = engine
        .run(RunCeremonyInput::new(
            ceremony_id.clone(),
            definition,
            CeremonyContext::empty(),
            LeaseOwnerId::new("pattern-behavior-host").unwrap(),
            DurationMs::from_millis(60_000),
            "pattern-test",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    assert_eq!(output.instance().current_state().as_str(), "COMPLETED");
    (engine, ceremony_id)
}

async fn status(engine: &EmbeddedMade, id: &CeremonyId, step: &str) -> StepStatus {
    engine
        .instance(id)
        .await
        .unwrap()
        .step_record(&StepId::new(step).unwrap())
        .unwrap()
        .status()
}

#[tokio::test]
async fn group_chat_stops_early_or_reaches_its_cap_fallback() {
    let (early, early_id) = run("group-early", GROUP_CHAT, Scenario::GroupEarly).await;
    assert_eq!(
        status(&early, &early_id, "coordination_fallback").await,
        StepStatus::Pending
    );
    assert_eq!(
        status(&early, &early_id, "coordination_speak_1").await,
        StepStatus::Pending
    );
    let (capped, capped_id) = run("group-cap", GROUP_CHAT, Scenario::GroupCap).await;
    assert_eq!(
        status(&capped, &capped_id, "coordination_fallback").await,
        StepStatus::Completed
    );
}

#[tokio::test]
async fn maker_checker_passes_on_iteration_two_or_escalates_after_the_cap() {
    let (passed, passed_id) = run("maker-pass", MAKER_CHECKER, Scenario::MakerSecond).await;
    assert_eq!(
        status(&passed, &passed_id, "coordination_fallback").await,
        StepStatus::Pending
    );
    let (capped, capped_id) = run("maker-cap", MAKER_CHECKER, Scenario::MakerCap).await;
    assert_eq!(
        status(&capped, &capped_id, "coordination_fallback").await,
        StepStatus::Completed
    );
}

#[tokio::test]
async fn handoff_routes_to_the_selected_specialist_and_resolves() {
    let (engine, id) = run("handoff-resolved", HANDOFF, Scenario::HandoffResolved).await;
    assert_eq!(
        status(&engine, &id, "coordination_security").await,
        StepStatus::Completed
    );
    assert_eq!(
        status(&engine, &id, "coordination_human").await,
        StepStatus::Pending
    );
}

#[tokio::test]
async fn handoff_bounces_within_its_cap_and_lands_on_the_human_exit() {
    let definition = CeremonyDefinitionYaml::parse_str(HANDOFF).unwrap();
    assert_eq!(definition.max_bounces().unwrap().get(), 2);
    let engine = EmbeddedMade::builder()
        .with_step_handler(Arc::new(ScenarioHandler(Scenario::HandoffBounce)))
        .build();
    let id = CeremonyId::new("handoff-bounce").unwrap();
    let _ = engine
        .run(RunCeremonyInput::new(
            id.clone(),
            definition,
            CeremonyContext::empty(),
            LeaseOwnerId::new("pattern-behavior-host").unwrap(),
            DurationMs::from_millis(60_000),
            "pattern-test",
            AuditActorKind::Service,
        ))
        .await;
    let paused = engine.instance(&id).await.unwrap();
    let (source, owner) = if paused.current_state().as_str() == "COORDINATION_OPS" {
        ("ops", "OPS")
    } else {
        ("security", "SECURITY")
    };
    engine
        .approve_guard(ApproveCeremonyGuardInput::new(
            id.clone(),
            GuardName::new(format!("coordination_{source}_to_human_human")).unwrap(),
            RoleId::new("HUMAN").unwrap(),
            AuditActorKind::Human,
        ))
        .await
        .unwrap();
    engine
        .apply_transition(ApplyCeremonyTransitionInput::new(
            id.clone(),
            RoleId::new(owner).unwrap(),
            AuditActorKind::Agent,
            TransitionTrigger::new(format!("coordination_{source}_to_human")).unwrap(),
        ))
        .await
        .unwrap();
    let instance = engine.instance(&id).await.unwrap();
    assert_eq!(instance.current_state().as_str(), "COORDINATION_HUMAN");
    assert_eq!(
        instance
            .step_record(&StepId::new("coordination_human").unwrap())
            .unwrap()
            .status(),
        StepStatus::Pending
    );
}

#[tokio::test]
async fn magentic_reports_completed_and_stalled_ledgers_from_the_stream() {
    let (completed, completed_id) = run("ledger-done", MAGENTIC, Scenario::LedgerCompleted).await;
    let report = completed
        .report(GenerateCeremonyReportInput::new(vec![completed_id], None).unwrap())
        .await
        .unwrap();
    assert!(
        report.markdown().contains("task-a"),
        "{}",
        report.markdown()
    );
    let (stalled, stalled_id) = run("ledger-stalled", MAGENTIC, Scenario::LedgerStalled).await;
    assert_eq!(
        status(&stalled, &stalled_id, "coordination_fallback").await,
        StepStatus::Completed
    );
    let report = stalled
        .report(GenerateCeremonyReportInput::new(vec![stalled_id], None).unwrap())
        .await
        .unwrap();
    assert!(
        report.markdown().contains("blocked"),
        "{}",
        report.markdown()
    );
}
