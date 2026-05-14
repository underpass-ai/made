//! Integration test: Chain 2 against the in-process `GrpcFixture`.
//!
//! Drives Strict mode against the canonical Report schema. The
//! NoopAgent emits free-form text, so the `JsonSchemaValidator` (and
//! the always-on `JsonObjectOutputValidator`) reject every candidate
//! and the use case returns `NoValidProposal`, which the gRPC mapper
//! surfaces as `Code::FailedPrecondition` whose message mentions the
//! contract id.
//!
//! That IS the assertion this chain makes against today's stack:
//! `report_contract_rejects_freeform_text` → Passed, with
//! `report_payload_validates` → Skipped (positive path needs a
//! stub-LLM that emits structured JSON).
//!
//! Uses `run_chain_2_with_schema` so the test doesn't have to mutate
//! `CHOREO_REPORT_SCHEMA_PATH` (env mutation leaks across the
//! concurrent test runner).

use std::sync::Arc;

use choreo_consumer_smoke::outcome::AssertionStatus;
use choreo_consumer_smoke::{run_chain_2_with_schema, Harness, HarnessConfig};
use choreo_core::entities::Council;
use choreo_core::ports::AgentDescriptor;
use choreo_core::value_objects::{AgentId, AgentKind, Attributes, CouncilId, Specialty};
use choreo_tests_integration::grpc_fixture::GrpcFixture;
use time::OffsetDateTime;

const SPECIALTY: &str = "triage";
const CONTRACT_ID: &str = "consumer-smoke-chain2";
const REPORT_SCHEMA_PATH: &str = "../../api/examples/output-contracts/report.schema.json";

async fn seed_agent(fixture: &GrpcFixture) -> AgentId {
    let id = AgentId::new("agent-chain2").unwrap();
    let descriptor = AgentDescriptor {
        id: id.clone(),
        specialty: Specialty::new(SPECIALTY).unwrap(),
        kind: AgentKind::new("noop").unwrap(),
        attributes: Attributes::empty(),
    };
    let factory = Arc::new(choreo_adapters::agents::DispatchingAgentFactory::new());
    let usecase = choreo_app::usecases::RegisterAgentUseCase::new(factory, fixture.agents.clone());
    usecase.execute(descriptor).await.unwrap();
    id
}

async fn seed_council(fixture: &GrpcFixture, agent_id: AgentId) {
    let council = Council::new(
        CouncilId::new("council-chain2").unwrap(),
        Specialty::new(SPECIALTY).unwrap(),
        vec![agent_id],
        OffsetDateTime::now_utc(),
    )
    .unwrap();
    fixture.councils.register(council).await.unwrap();
}

fn load_report_schema() -> String {
    // Tests run from the crate directory; the schema lives at the
    // repo root.
    std::fs::read_to_string(REPORT_SCHEMA_PATH).unwrap_or_else(|err| {
        panic!(
            "could not read Report schema at {REPORT_SCHEMA_PATH:?} (cwd={}): {err}",
            std::env::current_dir().unwrap_or_default().display()
        )
    })
}

#[tokio::test]
async fn chain2_strict_rejection_against_fixture_passes() {
    let fixture = GrpcFixture::start().await;
    let agent_id = seed_agent(&fixture).await;
    seed_council(&fixture, agent_id).await;
    let schema = load_report_schema();

    let mut harness = Harness::from_parts(fixture.channel.clone(), None);
    let cfg = HarnessConfig {
        endpoint: "in-process".to_owned(),
        nats_url: None,
        specialty: SPECIALTY.to_owned(),
        contract_id: CONTRACT_ID.to_owned(),
        connect_budget: std::time::Duration::from_secs(1),
    };

    let outcome = run_chain_2_with_schema(&mut harness, &cfg, &schema)
        .await
        .expect("run_chain_2_with_schema should not error");

    assert!(
        outcome.passed(),
        "chain2 should pass; assertions={:#?}",
        outcome.assertions
    );
    assert_eq!(
        outcome.failed_count(),
        0,
        "no assertion should fail; got {:#?}",
        outcome.assertions
    );

    // The rejection-path assertion MUST be Passed and the positive
    // assertion MUST be Skipped against today's NoopAgent stack.
    let by_name = |target: &str| {
        outcome
            .assertions
            .iter()
            .find(|a| a.name == target)
            .unwrap_or_else(|| panic!("missing assertion {target:?}: {:#?}", outcome.assertions))
    };
    assert!(
        by_name("report_schema_registered").is_passed(),
        "report_schema_registered should be Passed; got {:?}",
        by_name("report_schema_registered").status
    );
    assert!(
        by_name("report_contract_rejects_freeform_text").is_passed(),
        "rejection assertion should be Passed; got {:?}",
        by_name("report_contract_rejects_freeform_text").status
    );
    let validates = by_name("report_payload_validates");
    assert!(
        matches!(validates.status, AssertionStatus::Skipped { .. }),
        "report_payload_validates should be Skipped (no structured-JSON agent in fixture); got {:?}",
        validates.status
    );
}
