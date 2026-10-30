//! Authoring a protocol over gRPC.
//!
//! Checking a draft, being told what is wrong with it, and putting a
//! version in the catalogue. The first two answer about the YAML in
//! the request and leave no trace; the third is what makes a version
//! something an instance can be bound to.

use made_adapters::yaml::ProtocolDefinitionYaml;
use made_core::value_objects::{
    GuardCondition, GuardName, RoleAction, RoleId, StepId, TransitionTrigger,
};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    ExplainProtocolDraftRequest, PublishProtocolDefinitionRequest, StartPublishedProtocolRequest,
    ValidateProtocolDraftRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use tonic::Code;

const INTEGRATOR_DELIVERY: &str =
    include_str!("../../../docs/authoring/examples/integrator-delivery.yaml");

const PUBLISHABLE_PROTOCOL: &str = r#"
version: "1.0"
name: "authored_protocol"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: embedded_noop
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#;

/// The same protocol and version, with a materially different graph.
const ALTERED_PROTOCOL: &str = r#"
version: "1.0"
name: "authored_protocol"
states:
  - id: OPEN
    initial: true
  - id: CANCELLED
    terminal: true
transitions:
  - from: OPEN
    to: CANCELLED
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: embedded_noop
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#;

/// Its only transition leads somewhere it never declared, and its one
/// role is allowed to run a step that does not exist.
const BROKEN_PROTOCOL: &str = r#"
version: "1.0"
name: "broken_protocol"
states:
  - id: DRAFTING
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: DRAFTING
    to: NOWHERE
    trigger: finish
steps: []
roles:
  - id: FACILITATOR
    allowed_actions:
      - missing_step
"#;

#[tokio::test]
async fn a_sound_draft_validates_and_a_broken_one_says_where_it_is_broken() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let sound = client
        .validate_protocol_draft(ValidateProtocolDraftRequest {
            definition_yaml: PUBLISHABLE_PROTOCOL.to_owned(),
        })
        .await
        .expect("ValidateProtocolDraft should succeed")
        .into_inner();

    assert_eq!(sound.protocol, "authored_protocol");
    assert_eq!(sound.version, "1.0");
    assert!(sound.publishable);
    assert_eq!(sound.error_count, 0);

    let broken = client
        .validate_protocol_draft(ValidateProtocolDraftRequest {
            definition_yaml: BROKEN_PROTOCOL.to_owned(),
        })
        .await
        .expect("validating a broken draft is an answer, not an error")
        .into_inner();

    assert!(!broken.publishable);
    assert_eq!(broken.error_count, 2);
    assert_eq!(broken.findings.len(), 2);
    assert!(broken
        .findings
        .iter()
        .all(|finding| finding.severity == "error"));

    // The locus names the element at fault with the same structure the
    // in-process surface carries, so one client can read either.
    let transition = broken
        .findings
        .iter()
        .find_map(|finding| finding.locus.as_ref())
        .expect("a finding should point at an element");
    assert!(transition.fields.contains_key("kind"));
}

#[test]
fn integrator_example_keeps_integration_and_human_approval_separate() {
    let definition = ProtocolDefinitionYaml::parse_str(INTEGRATOR_DELIVERY)
        .expect("the executable Integrator example must parse and publish");
    let integrator = definition
        .roles()
        .get(&RoleId::new("INTEGRATOR").unwrap())
        .expect("the example declares an Integrator role");

    assert!(integrator.allowed_actions().contains(&RoleAction::step(
        StepId::new("integrate_delivery").unwrap()
    )));
    assert!(integrator
        .allowed_actions()
        .contains(&RoleAction::transition(
            TransitionTrigger::new("integration_ready").unwrap()
        )));
    assert!(!integrator
        .allowed_actions()
        .contains(&RoleAction::transition(
            TransitionTrigger::new("approve_delivery").unwrap(),
        )));
    assert!(matches!(
        definition
            .guards()
            .get(&GuardName::new("human_approved").unwrap())
            .expect("the final transition has a named human guard")
            .condition(),
        GuardCondition::HumanApproval
    ));
}

#[tokio::test]
async fn a_draft_that_cannot_be_parsed_is_an_error_not_an_empty_report() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let status = client
        .validate_protocol_draft(ValidateProtocolDraftRequest {
            definition_yaml: "this is not a protocol".to_owned(),
        })
        .await
        .expect_err("unparseable YAML has no findings to report");

    assert_ne!(status.code(), Code::Unknown);
}

#[tokio::test]
async fn explaining_a_draft_says_what_it_declares_and_what_blocks_it() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let explanation = client
        .explain_protocol_draft(ExplainProtocolDraftRequest {
            definition_yaml: BROKEN_PROTOCOL.to_owned(),
        })
        .await
        .expect("ExplainProtocolDraft should succeed")
        .into_inner();

    assert!(!explanation.publishable);
    let summary = explanation.summary.expect("an explanation should count");
    assert_eq!(summary.states, 2);
    assert_eq!(summary.initial_states, 1);
    assert_eq!(summary.terminal_states, 1);
    assert_eq!(summary.transitions, 1);

    assert!(explanation.narrative[0].contains("`broken_protocol` declares 2 states"));
    assert!(explanation
        .narrative
        .iter()
        .any(|line| line.contains("block publication")));
    // Each defect names its element in words. A serialized object
    // quoted into the middle of a sentence is prose only by accident.
    assert!(explanation
        .narrative
        .iter()
        .any(|line| line.contains(" — at ") && !line.contains('{')));
}

#[tokio::test]
async fn publishing_is_idempotent_for_the_same_content_and_refuses_different_content() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let published = client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: PUBLISHABLE_PROTOCOL.to_owned(),
        })
        .await
        .expect("PublishProtocolDefinition should succeed")
        .into_inner();

    assert_eq!(published.outcome, "published");
    assert_eq!(published.protocol, "authored_protocol");
    assert_eq!(published.version, "1.0");
    assert!(!published.digest.is_empty());

    // The same content again is not a failure: an author retrying a
    // call they never saw answered must not be told they broke
    // something.
    let again = client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: PUBLISHABLE_PROTOCOL.to_owned(),
        })
        .await
        .expect("republishing identical content should succeed")
        .into_inner();

    assert_eq!(again.outcome, "already_published");
    assert_eq!(again.digest, published.digest);

    // Different content under the same version is refused, and both
    // digests come back so the caller can see what differed rather
    // than being told only that something did.
    let occupied = client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: ALTERED_PROTOCOL.to_owned(),
        })
        .await
        .expect("a refused publication is an answer, not a transport error")
        .into_inner();

    assert_eq!(occupied.outcome, "version_occupied");
    assert_eq!(occupied.published_digest, published.digest);
    assert_ne!(occupied.offered_digest, published.digest);
}

#[tokio::test]
async fn a_published_version_is_something_an_instance_can_be_started_from() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let published = client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: PUBLISHABLE_PROTOCOL.to_owned(),
        })
        .await
        .expect("PublishProtocolDefinition should succeed")
        .into_inner();

    // The whole point of publishing: this is now a version a session
    // can be bound to, over the same connection, with no other step.
    let started = client
        .start_published_protocol(StartPublishedProtocolRequest {
            protocol_id: "authored-session".to_owned(),
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol: "authored_protocol".to_owned(),
            version: "1.0".to_owned(),
            context: None,
            budget_limits: None,
        })
        .await
        .expect("StartPublishedProtocol should succeed")
        .into_inner()
        .instance
        .expect("a started protocol must come back");

    assert_eq!(started.bound_definition_digest, published.digest);
    assert_eq!(started.current_state, "OPEN");
    assert_eq!(started.next_step_id, "work");
}
