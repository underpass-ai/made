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

#[derive(Debug, Default)]
struct PatternHandler;

#[async_trait]
impl CeremonyStepHandlerPort for PatternHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let id = request.step_id().as_str();
        let fields = if id.ends_with("check_1") {
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

async fn assert_runs_on_both_editions(name: &str, yaml: &str, expected_state: &str) {
    let handler = Arc::new(PatternHandler);
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
    assert_eq!(remote.final_state, expected_state);

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
    assert_eq!(local.instance().current_state().as_str(), expected_state);
}

#[tokio::test]
async fn concurrent_review_runs_through_real_rpc_and_embedded_facade() {
    assert_runs_on_both_editions("concurrent-review", CONCURRENT_REVIEW, "COMPLETED").await;
}

#[tokio::test]
async fn incident_review_runs_through_real_rpc_and_embedded_facade() {
    assert_runs_on_both_editions("incident-review", INCIDENT_REVIEW, "COMPLETED").await;
}
