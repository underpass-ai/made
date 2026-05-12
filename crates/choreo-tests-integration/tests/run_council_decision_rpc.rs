//! Epic 9 end-to-end: drive `RunCouncilDecision` over the in-process
//! gRPC fixture.
//!
//! The fixture wires the real `ChoreographerGrpcService` (in-memory
//! adapters, noop executor, noop messaging) over a localhost
//! `tokio::net::TcpListener` and hands the test a `tonic::Channel`.
//! The test seeds state through the registry ports (so it never
//! depends on the contract / council CRUD RPCs being correct) and
//! asserts on the RPC surface.
//!
//! The `NoopAgent`'s proposal is plain text ("noop-proposal[...]"),
//! which never satisfies a `JsonObject`-format contract. That
//! constraint shapes the test matrix:
//!
//! * Strict mode + any structured-output contract → `NoValidProposal`
//!   → `Code::FailedPrecondition`.
//! * Warn mode + same contract → winner returned with
//!   `validation.passed=false`; candidates non-empty.
//! * Unknown `contract_id` → `Code::NotFound`.
//! * Selector unset → `Code::InvalidArgument` (mapper rejects before
//!   the use case runs).

use std::sync::Arc;

use choreo_core::entities::Council;
use choreo_core::ports::AgentDescriptor;
use choreo_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, OutputContract, OutputFormat, Specialty,
};
use choreo_proto::v1 as pb;
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_tests_integration::grpc_fixture::GrpcFixture;
use time::OffsetDateTime;
use tonic::Code;

const SPECIALTY: &str = "triage";
const CONTRACT_ID: &str = "epic-9-contract";

async fn seed_one_agent(fixture: &GrpcFixture) -> AgentId {
    let id = AgentId::new("agent-epic-9-0").unwrap();
    let descriptor = AgentDescriptor {
        id: id.clone(),
        specialty: Specialty::new(SPECIALTY).unwrap(),
        kind: AgentKind::new("noop").unwrap(),
        attributes: Attributes::empty(),
    };
    let factory = Arc::new(choreo_adapters::agents::DispatchingAgentFactory::new());
    let usecase = choreo_app::usecases::RegisterAgentUseCase::new(factory, fixture.agents.clone());
    usecase
        .execute(descriptor)
        .await
        .expect("register agent should succeed");
    id
}

async fn seed_one_council(fixture: &GrpcFixture, agent_id: AgentId) {
    let council = Council::new(
        CouncilId::new("council-epic-9").unwrap(),
        Specialty::new(SPECIALTY).unwrap(),
        vec![agent_id],
        OffsetDateTime::now_utc(),
    )
    .expect("council construction should succeed");
    fixture
        .councils
        .register(council)
        .await
        .expect("council registration should succeed");
}

async fn seed_contract(fixture: &GrpcFixture) {
    let contract = OutputContract::new(CONTRACT_ID, OutputFormat::JsonObject, Default::default())
        .expect("contract construction should succeed");
    fixture
        .contracts
        .register(contract)
        .await
        .expect("contract registration should succeed");
}

#[tokio::test]
async fn run_council_decision_strict_with_noop_fails_with_no_valid_proposal() {
    let fixture = GrpcFixture::start().await;
    let agent_id = seed_one_agent(&fixture).await;
    seed_one_council(&fixture, agent_id).await;
    seed_contract(&fixture).await;

    let mut client = ChoreographerServiceClient::new(fixture.channel.clone());
    let request = pb::RunCouncilDecisionRequest {
        contract_id: CONTRACT_ID.to_owned(),
        external_context: None,
        validation_mode: pb::ValidationMode::Strict as i32,
        metadata: None,
        description: "decide whether to escalate".to_owned(),
        selector: Some(pb::run_council_decision_request::Selector::Specialty(
            SPECIALTY.to_owned(),
        )),
    };

    let err = client.run_council_decision(request).await.expect_err(
        "strict mode should fail when noop proposals can't satisfy a JSON-object contract",
    );
    assert_eq!(err.code(), Code::FailedPrecondition);
    assert!(err.message().contains(CONTRACT_ID));
}

#[tokio::test]
async fn run_council_decision_warn_returns_failing_winner() {
    let fixture = GrpcFixture::start().await;
    let agent_id = seed_one_agent(&fixture).await;
    seed_one_council(&fixture, agent_id).await;
    seed_contract(&fixture).await;

    let mut client = ChoreographerServiceClient::new(fixture.channel.clone());
    let request = pb::RunCouncilDecisionRequest {
        contract_id: CONTRACT_ID.to_owned(),
        external_context: None,
        validation_mode: pb::ValidationMode::Warn as i32,
        metadata: None,
        description: "decide whether to escalate".to_owned(),
        selector: Some(pb::run_council_decision_request::Selector::Specialty(
            SPECIALTY.to_owned(),
        )),
    };

    let response = client
        .run_council_decision(request)
        .await
        .expect("warn mode should always return a response")
        .into_inner();

    assert!(!response.task_id.is_empty(), "task_id should be populated");
    assert!(response.winner.is_some(), "winner must be returned");
    assert!(
        !response.candidates.is_empty(),
        "candidates must include the seeded NoopAgent's proposal"
    );
    let validation = response
        .validation
        .as_ref()
        .expect("validation summary must be present");
    assert!(
        !validation.passed,
        "noop proposal cannot satisfy a JSON-object contract"
    );
    assert_eq!(
        validation.candidates_passed, 0,
        "no candidate should satisfy the contract"
    );
    assert!(validation.candidates_total >= 1);
    assert_eq!(
        response.validation_mode,
        pb::ValidationMode::Warn as i32,
        "validation_mode must be echoed back"
    );
    // duration_ms isn't strictly > 0 on very fast hardware, so we only
    // assert the field exists and is non-negative (u64 satisfies that
    // by construction). Tighter timing is brittle in CI.
    let _ = response.duration_ms;
}

#[tokio::test]
async fn run_council_decision_with_unknown_contract_id_returns_not_found() {
    let fixture = GrpcFixture::start().await;
    let agent_id = seed_one_agent(&fixture).await;
    seed_one_council(&fixture, agent_id).await;
    // Deliberately do NOT seed any contract.

    let mut client = ChoreographerServiceClient::new(fixture.channel.clone());
    let request = pb::RunCouncilDecisionRequest {
        contract_id: "does-not-exist".to_owned(),
        external_context: None,
        validation_mode: pb::ValidationMode::Strict as i32,
        metadata: None,
        description: "decide".to_owned(),
        selector: Some(pb::run_council_decision_request::Selector::Specialty(
            SPECIALTY.to_owned(),
        )),
    };

    let err = client
        .run_council_decision(request)
        .await
        .expect_err("unknown contract id should be rejected");
    assert_eq!(err.code(), Code::NotFound);
}

#[tokio::test]
async fn run_council_decision_without_selector_returns_invalid_argument() {
    let fixture = GrpcFixture::start().await;

    let mut client = ChoreographerServiceClient::new(fixture.channel.clone());
    let request = pb::RunCouncilDecisionRequest {
        contract_id: CONTRACT_ID.to_owned(),
        external_context: None,
        validation_mode: pb::ValidationMode::Strict as i32,
        metadata: None,
        description: "decide".to_owned(),
        selector: None,
    };

    let err = client
        .run_council_decision(request)
        .await
        .expect_err("missing selector should be rejected by the mapper");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("selector"));
}
