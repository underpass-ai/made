//! Integration test: Chain 1 against the in-process `GrpcFixture`.
//!
//! Seeds a noop agent + a permissive Warn-mode-friendly contract,
//! builds a `Harness` directly from the fixture's `tonic::Channel`
//! (so the test doesn't pay the `Harness::connect` retry budget
//! against a server that's already accepting), and asserts:
//!
//! - `outcome.passed()` — the chain reports a green run.
//! - At least the 3 expected RPC-shape assertions Passed.
//! - The 3 NATS-coupled / bundle-seam assertions Skipped.
//! - No assertion Failed.
//!
//! The NoopAgent emits free-form text. `JsonObjectOutputValidator`
//! always fails on that, so the council's deliberation completes
//! with a winner whose validation passed=false (the Warn-mode
//! escalation signal). The chain treats that as a pass — it asserted
//! that the response carried a winner + a validation summary +
//! candidates, which is the consumer-visible contract.

use std::collections::BTreeMap;
use std::sync::Arc;

use made_consumer_smoke::{run_chain_1, Harness, HarnessConfig};
use made_core::entities::Council;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, OutputContract, OutputFormat, Specialty,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use time::OffsetDateTime;

const SPECIALTY: &str = "triage";
const CONTRACT_ID: &str = "consumer-smoke-chain1";

async fn seed_agent(fixture: &GrpcFixture) -> AgentId {
    let id = AgentId::new("agent-chain1").unwrap();
    let descriptor = AgentDescriptor {
        id: id.clone(),
        specialty: Specialty::new(SPECIALTY).unwrap(),
        kind: AgentKind::new("noop").unwrap(),
        attributes: Attributes::empty(),
    };
    let factory = Arc::new(made_adapters::agents::DispatchingAgentFactory::new());
    let usecase = made_app::usecases::RegisterAgentUseCase::new(factory, fixture.agents.clone());
    usecase
        .execute(descriptor)
        .await
        .expect("register agent should succeed");
    id
}

async fn seed_council(fixture: &GrpcFixture, agent_id: AgentId) {
    let council = Council::new(
        CouncilId::new("council-chain1").unwrap(),
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
    // Permissive JsonObject contract. The NoopAgent's free-form text
    // won't satisfy it, but Warn mode still surfaces a winner with
    // `validation.passed = false` — exactly the consumer-facing
    // signal Chain 1 asserts on.
    let contract = OutputContract::new(CONTRACT_ID, OutputFormat::JsonObject, BTreeMap::new())
        .expect("contract construction should succeed");
    fixture
        .contracts
        .register(contract)
        .await
        .expect("contract registration should succeed");
}

#[tokio::test]
async fn chain1_warn_against_fixture_passes_without_nats() {
    let fixture = GrpcFixture::start().await;
    let agent_id = seed_agent(&fixture).await;
    seed_council(&fixture, agent_id).await;
    seed_contract(&fixture).await;

    let mut harness = Harness::from_parts(fixture.channel.clone(), None);
    let cfg = HarnessConfig {
        endpoint: "in-process".to_owned(),
        nats_url: None,
        specialty: SPECIALTY.to_owned(),
        contract_id: CONTRACT_ID.to_owned(),
        connect_budget: std::time::Duration::from_secs(1),
    };

    let outcome = run_chain_1(&mut harness, &cfg)
        .await
        .expect("run_chain_1 should not error");

    assert!(
        outcome.passed(),
        "chain1 should pass; got assertions={:#?}",
        outcome.assertions
    );
    assert_eq!(
        outcome.failed_count(),
        0,
        "no assertion should fail; got {:#?}",
        outcome.assertions
    );
    assert!(
        outcome.passed_count() >= 3,
        "expected ≥3 Passed (rpc_returned_winner, validation_summary_present, \
         candidates_non_empty); got passed={} assertions={:#?}",
        outcome.passed_count(),
        outcome.assertions
    );
    assert!(
        outcome.skipped_count() >= 3,
        "expected ≥3 Skipped (bundle_seam_documented, trigger_envelope_observed, \
         causal_metadata_propagated); got skipped={} assertions={:#?}",
        outcome.skipped_count(),
        outcome.assertions
    );

    // Pin the specific assertion names so a refactor that renames one
    // is caught here.
    let names: Vec<&str> = outcome.assertions.iter().map(|a| a.name).collect();
    for required in [
        "rpc_returned_winner",
        "validation_summary_present",
        "candidates_non_empty",
        "bundle_seam_documented",
        "trigger_envelope_observed",
        "causal_metadata_propagated",
    ] {
        assert!(
            names.contains(&required),
            "missing assertion {required:?}; got {names:?}"
        );
    }

    // The chain should record the task id from the Warn response.
    assert!(
        outcome.task_id.is_some(),
        "task_id should be populated by Warn mode"
    );
    // validation_passed should be Some(false) — the NoopAgent's
    // free-form text fails the JsonObject contract, which is the
    // escalation signal Warn mode surfaces.
    assert_eq!(outcome.validation_passed, Some(false));
}
