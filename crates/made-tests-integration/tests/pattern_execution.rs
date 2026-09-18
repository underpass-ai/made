use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::RunCeremonyInput;
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, LeaseOwnerId, StepOutput,
    StepResult,
};
use made_embedded::EmbeddedMade;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::RunCeremonyRequest;
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::json;

const CONCURRENT_REVIEW: &str =
    include_str!("../../../tests/e2e/ceremonies/concurrent-review.yaml");
const INCIDENT_REVIEW: &str = include_str!("../../../tests/e2e/ceremonies/incident_review.yaml");
const GROUP_CHAT: &str = include_str!("../../../api/examples/ceremonies/fragments/group_chat.yaml");
const MAKER_CHECKER: &str =
    include_str!("../../../api/examples/ceremonies/fragments/maker_checker.yaml");
const HANDOFF: &str = include_str!("../../../api/examples/ceremonies/fragments/handoff.yaml");
const MAGENTIC: &str = include_str!("../../../api/examples/ceremonies/fragments/magentic.yaml");

#[derive(Debug, Clone, Copy, Default)]
enum PatternScenario {
    #[default]
    Preset,
    GroupChatEarlyExit,
    MakerCheckerSecondPass,
    HandoffResolved,
    MagenticCompleted,
}

#[derive(Debug, Default)]
struct PatternHandler(PatternScenario);

#[async_trait]
impl CeremonyStepHandlerPort for PatternHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let id = request.step_id().as_str();
        let fields = if matches!(self.0, PatternScenario::GroupChatEarlyExit)
            && id.ends_with("manage_1")
        {
            BTreeMap::from([
                ("done".to_owned(), json!(true)),
                ("instructions".to_owned(), json!("record the outcome")),
            ])
        } else if matches!(self.0, PatternScenario::HandoffResolved) && id.ends_with("_ops") {
            BTreeMap::from([
                ("resolved".to_owned(), json!(false)),
                ("handoff_to".to_owned(), json!("SECURITY")),
            ])
        } else if matches!(self.0, PatternScenario::HandoffResolved) && id.ends_with("_security") {
            BTreeMap::from([
                ("resolved".to_owned(), json!(true)),
                ("handoff_to".to_owned(), json!(null)),
            ])
        } else if matches!(self.0, PatternScenario::MagenticCompleted) && id.ends_with("_plan") {
            BTreeMap::from([(
                "ledger".to_owned(),
                json!({"tasks":[{"id":"task-a","status":"open"}]}),
            )])
        } else if matches!(self.0, PatternScenario::MagenticCompleted) && id.contains("_pick_") {
            BTreeMap::from([("owner".to_owned(), json!("OPS"))])
        } else if matches!(self.0, PatternScenario::MagenticCompleted) && id.ends_with("_update_1")
        {
            BTreeMap::from([
                ("done".to_owned(), json!(true)),
                ("stalled".to_owned(), json!(false)),
                (
                    "ledger".to_owned(),
                    json!({"tasks":[{"id":"task-a","status":"done"}]}),
                ),
            ])
        } else if id.ends_with("check_1") {
            BTreeMap::from([("approved".to_owned(), json!(false))])
        } else if id.ends_with("check_2") {
            BTreeMap::from([("approved".to_owned(), json!(true))])
        } else if id.contains("collect") || id == "synthesize_reviews" {
            BTreeMap::from([
                (
                    "summary".to_owned(),
                    json!("all sibling reviews synthesized"),
                ),
                ("winner_content".to_owned(), json!("accepted synthesis")),
            ])
        } else {
            BTreeMap::from([("result".to_owned(), json!(format!("completed {id}")))])
        };
        StepResult::completed(StepOutput::new(Attributes::new(fields)?))
    }
}

fn context(yaml: &str) -> prost_types::Struct {
    let fields = if yaml == CONCURRENT_REVIEW {
        [(
            "draft".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "candidate".to_owned(),
                )),
            },
        )]
        .into_iter()
        .collect()
    } else {
        BTreeMap::new()
    };
    prost_types::Struct { fields }
}

async fn assert_runs_on_both_editions(
    name: &str,
    yaml: &str,
    scenario: PatternScenario,
    required_steps: &[&str],
    skipped_steps: &[&str],
) {
    let handler = Arc::new(PatternHandler(scenario));
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_step_handler(handler.clone())).await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let remote = client
        .run_ceremony(RunCeremonyRequest {
            actor_id: "pattern-test".to_owned(),
            actor_kind: "service".to_owned(),
            ceremony_id: format!("remote-{name}"),
            definition_yaml: yaml.to_owned(),
            context: Some(context(yaml)),
            lease_owner_id: "remote-pattern-host".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .expect("real RPC executes the pattern")
        .into_inner();
    assert!(remote.completed, "{remote:?}");
    assert_eq!(remote.final_state, "COMPLETED");
    for step_id in required_steps {
        assert!(
            remote
                .steps
                .iter()
                .any(|step| step.step_id == *step_id && step.status == "COMPLETED"),
            "RPC path did not complete {step_id}: {:?}",
            remote.steps
        );
    }
    for step_id in skipped_steps {
        assert!(
            remote.steps.iter().all(|step| step.step_id != *step_id),
            "RPC path unexpectedly executed {step_id}: {:?}",
            remote.steps
        );
    }

    let definition = CeremonyDefinitionYaml::parse_str(yaml).unwrap();
    let embedded = EmbeddedMade::builder().with_step_handler(handler).build();
    let local = embedded
        .run(RunCeremonyInput::new(
            CeremonyId::new(format!("embedded-{name}")).unwrap(),
            definition,
            if yaml == CONCURRENT_REVIEW {
                CeremonyContext::new(
                    Attributes::new(BTreeMap::from([("draft".to_owned(), json!("candidate"))]))
                        .unwrap(),
                )
            } else {
                CeremonyContext::empty()
            },
            LeaseOwnerId::new("embedded-pattern-host").unwrap(),
            DurationMs::from_millis(60_000),
            "pattern-test",
            AuditActorKind::Service,
        ))
        .await
        .expect("embedded facade executes the pattern");
    assert!(local
        .instance()
        .is_completed(&CeremonyDefinitionYaml::parse_str(yaml).unwrap()));
    assert_eq!(local.instance().current_state().as_str(), "COMPLETED");
    for step_id in required_steps {
        assert_eq!(
            local
                .instance()
                .step_record(&made_core::value_objects::StepId::new(*step_id).unwrap())
                .unwrap()
                .status(),
            made_core::value_objects::StepStatus::Completed,
            "embedded path did not complete {step_id}"
        );
    }
    for step_id in skipped_steps {
        assert_eq!(
            local
                .instance()
                .step_record(&made_core::value_objects::StepId::new(*step_id).unwrap())
                .unwrap()
                .status(),
            made_core::value_objects::StepStatus::Pending,
            "embedded path unexpectedly executed {step_id}"
        );
    }
}

#[tokio::test]
async fn concurrent_review_runs_through_real_rpc_and_embedded_facade() {
    Box::pin(assert_runs_on_both_editions(
        "concurrent-review",
        CONCURRENT_REVIEW,
        PatternScenario::Preset,
        &["synthesize_reviews"],
        &[],
    ))
    .await;
}

#[tokio::test]
async fn incident_review_runs_through_real_rpc_and_embedded_facade() {
    Box::pin(assert_runs_on_both_editions(
        "incident-review",
        INCIDENT_REVIEW,
        PatternScenario::Preset,
        &["writeup_check_2"],
        &["writeup_fallback"],
    ))
    .await;
}

#[tokio::test]
async fn group_chat_fragment_exits_from_manager_over_rpc_and_embedded() {
    Box::pin(assert_runs_on_both_editions(
        "group-chat",
        GROUP_CHAT,
        PatternScenario::GroupChatEarlyExit,
        &["coordination_manage_1", "coordination_record"],
        &["coordination_select_1", "coordination_fallback"],
    ))
    .await;
}

#[tokio::test]
async fn maker_checker_fragment_accepts_second_pass_over_rpc_and_embedded() {
    Box::pin(assert_runs_on_both_editions(
        "maker-checker",
        MAKER_CHECKER,
        PatternScenario::MakerCheckerSecondPass,
        &[
            "coordination_check_1",
            "coordination_check_2",
            "coordination_deliver",
        ],
        &["coordination_fallback"],
    ))
    .await;
}

#[tokio::test]
async fn handoff_fragment_routes_to_security_over_rpc_and_embedded() {
    Box::pin(assert_runs_on_both_editions(
        "handoff",
        HANDOFF,
        PatternScenario::HandoffResolved,
        &[
            "coordination_ops",
            "coordination_security",
            "coordination_record",
        ],
        &["coordination_human"],
    ))
    .await;
}

#[tokio::test]
async fn magentic_fragment_completes_ledger_over_rpc_and_embedded() {
    Box::pin(assert_runs_on_both_editions(
        "magentic",
        MAGENTIC,
        PatternScenario::MagenticCompleted,
        &[
            "coordination_plan",
            "coordination_update_1",
            "coordination_record",
        ],
        &["coordination_update_2", "coordination_fallback"],
    ))
    .await;
}
