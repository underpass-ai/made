//! End-to-end runner.
//!
//! Connects to a running Choreographer over gRPC and executes a
//! selectable sequence of scenarios that only pass if the stack is
//! wired correctly. Intended to run either inside the docker-compose
//! stack or as a Kubernetes Job against a Helm-installed release.
//!
//! Exits 0 on success, non-zero on the first failed assertion.

use std::time::Duration;

use anyhow::{Context, Result};
use scenario_selection::{
    parse_scenario_selection, scenario_selection_summary, E2eScenario, SCENARIO_SELECTION_ENV,
};
use scenarios::{
    connect_with_retry, verify_causal_metadata_propagates_over_nats,
    verify_delete_missing_council_returns_false, verify_deliberate_returns_winner,
    verify_editorial_meeting_ceremony_diagram, verify_external_context_bundle_round_trips,
    verify_multi_agent_council_against_real_vllm, verify_orchestrate_invokes_runtime_executor,
    verify_orchestrate_rejects_proposal_violating_json_schema, verify_seeded_council_visible,
    verify_structured_output_against_stub_llm, verify_structured_output_against_vllm_kind,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod scenario_selection;
mod scenarios;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .compact()
        .init();

    let endpoint = std::env::var("CHOREOGRAPHER_ENDPOINT")
        .unwrap_or_else(|_| "http://choreographer:50055".to_owned());
    let seed_specialty =
        std::env::var("CHOREO_SEED_SPECIALTY").unwrap_or_else(|_| "triage".to_owned());
    let scenario_selection_raw = std::env::var(SCENARIO_SELECTION_ENV).ok();
    let selected_scenarios = parse_scenario_selection(scenario_selection_raw.as_deref())?;

    info!(
        env = SCENARIO_SELECTION_ENV,
        selected = scenario_selection_summary(&selected_scenarios),
        "selected E2E scenarios"
    );

    let mut client = connect_with_retry(&endpoint, Duration::from_secs(30)).await?;

    if selected_scenarios.contains(&E2eScenario::SeededCouncil) {
        info!("scenario 1: seeded council is visible");
        verify_seeded_council_visible(&mut client, &seed_specialty)
            .await
            .context("scenario 1 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::Deliberate) {
        info!("scenario 2: Deliberate on the seeded specialty returns a winner");
        verify_deliberate_returns_winner(&mut client, &seed_specialty)
            .await
            .context("scenario 2 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::DeleteMissingCouncil) {
        info!("scenario 3: DeleteCouncil on a missing specialty returns deleted=false");
        verify_delete_missing_council_returns_false(&mut client)
            .await
            .context("scenario 3 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::CausalMetadata) {
        info!("scenario 4: causal metadata propagates from inbound trigger to outbound bus event");
        verify_causal_metadata_propagates_over_nats(&seed_specialty)
            .await
            .context("scenario 4 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::RuntimeExecutor) {
        info!("scenario 5: Orchestrate routes the winner through the configured Runtime executor");
        verify_orchestrate_invokes_runtime_executor(&mut client, &seed_specialty)
            .await
            .context("scenario 5 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::StrictSchemaRejection) {
        info!("scenario 6: Orchestrate with a strict JSON Schema output contract rejects free-form proposals");
        verify_orchestrate_rejects_proposal_violating_json_schema(&mut client, &seed_specialty)
            .await
            .context("scenario 6 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::ExternalContextBundle) {
        info!(
            "scenario 7: ExternalContextBundle round-trips to the outbound DeliberationCompleted envelope"
        );
        verify_external_context_bundle_round_trips(&mut client, &seed_specialty)
            .await
            .context("scenario 7 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::OpenAiStructuredOutput) {
        info!(
            "scenario 8: structured-output Report contract passes against a stub OpenAI-shaped agent"
        );
        verify_structured_output_against_stub_llm(&mut client)
            .await
            .context("scenario 8 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::VllmStructuredOutput) {
        info!(
            "scenario 9: structured-output Report contract passes against the stub-llm via the vllm adapter"
        );
        verify_structured_output_against_vllm_kind(&mut client)
            .await
            .context("scenario 9 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::VllmRealMultiAgent) {
        info!("scenario 10: real vLLM multi-agent council returns a schema-valid Report winner");
        verify_multi_agent_council_against_real_vllm(&mut client)
            .await
            .context("scenario 10 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::CeremonyDiagram) {
        info!("scenario 11: four-role ceremony YAML renders a Mermaid conversation diagram");
        verify_editorial_meeting_ceremony_diagram().context("scenario 11 failed")?;
    }

    info!("E2E scenarios passed");
    Ok(())
}
