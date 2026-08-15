//! End-to-end runner.
//!
//! Connects to a running MADE over gRPC and executes a
//! selectable sequence of scenarios that only pass if the stack is
//! wired correctly. Intended to run either inside the docker-compose
//! stack or as a Kubernetes Job against a Helm-installed release.
//!
//! Exits 0 on success, non-zero on the first failed assertion.

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{Context, Result};
use made_proto::v1::made_service_client::MadeServiceClient;
use scenario_selection::{
    parse_scenario_selection, scenario_selection_summary, E2eScenario, SCENARIO_SELECTION_ENV,
};
use scenarios::{
    connect_with_retry, verify_causal_metadata_propagates_over_nats, verify_daily_standup_ceremony,
    verify_delete_missing_council_returns_false, verify_deliberate_returns_winner,
    verify_editorial_meeting_ceremony_against_vllm_kind, verify_editorial_meeting_ceremony_diagram,
    verify_external_context_bundle_round_trips, verify_multi_agent_council_against_real_vllm,
    verify_orchestrate_invokes_runtime_executor,
    verify_orchestrate_rejects_proposal_violating_json_schema, verify_seeded_council_visible,
    verify_speaker_talk_qa_ceremony, verify_sprint_planning_ceremony,
    verify_structured_output_against_stub_llm, verify_structured_output_against_vllm_kind,
    verify_technical_debate_ceremony,
};
use tonic::transport::Channel;
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

    let endpoint =
        std::env::var("MADE_ENDPOINT").unwrap_or_else(|_| "http://made:50055".to_owned());
    let seed_specialty =
        std::env::var("MADE_SEED_SPECIALTY").unwrap_or_else(|_| "triage".to_owned());
    let scenario_selection_raw = std::env::var(SCENARIO_SELECTION_ENV).ok();
    let selected_scenarios = parse_scenario_selection(scenario_selection_raw.as_deref())?;

    info!(
        env = SCENARIO_SELECTION_ENV,
        selected = scenario_selection_summary(&selected_scenarios),
        "selected E2E scenarios"
    );

    let mut client = connect_with_retry(&endpoint, Duration::from_secs(30)).await?;

    run_selected_scenarios(&mut client, &selected_scenarios, &seed_specialty).await?;

    info!("E2E scenarios passed");
    Ok(())
}

/// Run each selected scenario in order, failing on the first assertion
/// that does not hold.
async fn run_selected_scenarios(
    client: &mut MadeServiceClient<Channel>,
    selected_scenarios: &BTreeSet<E2eScenario>,
    seed_specialty: &str,
) -> Result<()> {
    if selected_scenarios.contains(&E2eScenario::SeededCouncil) {
        info!("scenario 1: seeded council is visible");
        verify_seeded_council_visible(client, seed_specialty)
            .await
            .context("scenario 1 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::Deliberate) {
        info!("scenario 2: Deliberate on the seeded specialty returns a winner");
        verify_deliberate_returns_winner(client, seed_specialty)
            .await
            .context("scenario 2 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::DeleteMissingCouncil) {
        info!("scenario 3: DeleteCouncil on a missing specialty returns deleted=false");
        verify_delete_missing_council_returns_false(client)
            .await
            .context("scenario 3 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::CausalMetadata) {
        info!("scenario 4: causal metadata propagates from inbound trigger to outbound bus event");
        verify_causal_metadata_propagates_over_nats(seed_specialty)
            .await
            .context("scenario 4 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::RuntimeExecutor) {
        info!("scenario 5: Orchestrate routes the winner through the configured Runtime executor");
        verify_orchestrate_invokes_runtime_executor(client, seed_specialty)
            .await
            .context("scenario 5 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::StrictSchemaRejection) {
        info!("scenario 6: Orchestrate with a strict JSON Schema output contract rejects free-form proposals");
        verify_orchestrate_rejects_proposal_violating_json_schema(client, seed_specialty)
            .await
            .context("scenario 6 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::ExternalContextBundle) {
        info!(
            "scenario 7: ExternalContextBundle round-trips to the outbound DeliberationCompleted envelope"
        );
        verify_external_context_bundle_round_trips(client, seed_specialty)
            .await
            .context("scenario 7 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::OpenAiStructuredOutput) {
        info!(
            "scenario 8: structured-output Report contract passes against a stub OpenAI-shaped agent"
        );
        verify_structured_output_against_stub_llm(client)
            .await
            .context("scenario 8 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::VllmStructuredOutput) {
        info!(
            "scenario 9: structured-output Report contract passes against the stub-llm via the vllm adapter"
        );
        verify_structured_output_against_vllm_kind(client)
            .await
            .context("scenario 9 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::VllmRealMultiAgent) {
        info!("scenario 10: real vLLM multi-agent council returns a schema-valid Report winner");
        verify_multi_agent_council_against_real_vllm(client)
            .await
            .context("scenario 10 failed")?;
    }

    run_ceremony_scenarios(client, selected_scenarios).await?;

    Ok(())
}

/// Run the selected ceremony scenarios — declarative meeting ceremonies
/// driven through the RunCeremony RPC.
async fn run_ceremony_scenarios(
    client: &mut MadeServiceClient<Channel>,
    selected_scenarios: &BTreeSet<E2eScenario>,
) -> Result<()> {
    if selected_scenarios.contains(&E2eScenario::CeremonyDiagram) {
        info!("scenario 11: four-role ceremony YAML renders a Mermaid conversation diagram");
        verify_editorial_meeting_ceremony_diagram(client)
            .await
            .context("scenario 11 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::CeremonyVllm) {
        info!("scenario 12: four-agent ceremony YAML executes through the vLLM adapter");
        verify_editorial_meeting_ceremony_against_vllm_kind(client)
            .await
            .context("scenario 12 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::DailyStandup) {
        info!("scenario 13: four-role daily standup threads each turn through the transcript");
        verify_daily_standup_ceremony(client)
            .await
            .context("scenario 13 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::TechnicalDebate) {
        info!("scenario 14: four-role technical debate argues, rebuts across rounds, and decides");
        verify_technical_debate_ceremony(client)
            .await
            .context("scenario 14 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::SprintPlanning) {
        info!("scenario 15: four-role sprint planning threads priorities into a committed scope");
        verify_sprint_planning_ceremony(client)
            .await
            .context("scenario 15 failed")?;
    }

    if selected_scenarios.contains(&E2eScenario::SpeakerTalkQa) {
        info!("scenario 16: speaker talk + Q&A grounds questions and answers in the talk");
        verify_speaker_talk_qa_ceremony(client)
            .await
            .context("scenario 16 failed")?;
    }

    Ok(())
}
