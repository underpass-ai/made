use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::run_council_decision_request::Selector as RunCouncilSelector;
use made_proto::v1::{
    AgentSummary, CreateCouncilRequest, OutputContract, OutputFormat, RegisterAgentRequest,
    RegisterContractRequest, RunCouncilDecisionRequest, ValidationMode,
};
use prost_types::value::Kind as PbKind;
use tonic::transport::Channel;
use tonic::Code;
use tracing::{info, warn};

use super::super::pb_struct_from_pairs;

/// Same end-to-end Report-contract success path as scenario 8, but
/// the council's agent is registered with `kind=vllm`. The vLLM
/// adapter sends the same `POST /v1/chat/completions` body shape the
/// OpenAI adapter does, so we point it at the existing `stub-llm`
/// sidecar — no second sidecar needed. Proves the vllm-shaped path
/// works in the same compose run that scenario 8 covers for the
/// openai-shaped path, closing Epic 11's provider-runner-merge
/// follow-up.
#[allow(clippy::too_many_lines)] // single end-to-end scenario; splitting fragments the assertion
pub(crate) async fn verify_structured_output_against_vllm_kind(
    client: &mut MadeServiceClient<Channel>,
) -> Result<()> {
    const CONTRACT_ID: &str = "scenario-9-report-vllm";
    // Must match the id pattern the CreateCouncil handler mints
    // (`agent-{specialty}-{i}`).
    const AGENT_ID: &str = "agent-report-vllm-0";
    const SPECIALTY: &str = "report-vllm";
    const STUB_ENDPOINT: &str = "http://stub-llm:8000";

    let schema_path = std::env::var("MADE_REPORT_SCHEMA_PATH")
        .unwrap_or_else(|_| "/etc/made/report.schema.json".to_owned());
    let schema_body = std::fs::read_to_string(&schema_path)
        .with_context(|| format!("read Report schema at {schema_path}"))?;
    let schema_value: serde_json::Value =
        serde_json::from_str(&schema_body).context("Report schema must parse as JSON")?;
    let compiled_schema = jsonschema::JSONSchema::compile(&schema_value)
        .map_err(|err| anyhow!("Report schema must compile: {err}"))?;

    let register_contract = client
        .register_contract(RegisterContractRequest {
            contract: Some(OutputContract {
                contract_id: CONTRACT_ID.to_owned(),
                format: OutputFormat::JsonObject as i32,
                fields: std::collections::HashMap::new(),
                json_schema: schema_body.clone(),
            }),
        })
        .await;
    match register_contract {
        Ok(_) => info!(
            contract_id = CONTRACT_ID,
            "RegisterContract ok (scenario 9)"
        ),
        Err(status)
            if matches!(
                status.code(),
                Code::AlreadyExists | Code::FailedPrecondition
            ) =>
        {
            info!(
                code = ?status.code(),
                contract_id = CONTRACT_ID,
                "RegisterContract tolerated (contract already present)"
            );
        }
        Err(status) => bail!("RegisterContract failed unexpectedly (scenario 9): {status}"),
    }

    // The vllm adapter does NOT read `provider.api_key`; only the
    // endpoint + model overrides are meaningful.
    let agent_attrs = pb_struct_from_pairs([
        (
            "provider.endpoint",
            PbKind::StringValue(STUB_ENDPOINT.to_owned()),
        ),
        (
            "provider.model",
            PbKind::StringValue("stub-report-vllm-v1".to_owned()),
        ),
    ]);
    let register_agent = client
        .register_agent(RegisterAgentRequest {
            specialty: SPECIALTY.to_owned(),
            agent: Some(AgentSummary {
                agent_id: AGENT_ID.to_owned(),
                specialty: SPECIALTY.to_owned(),
                kind: "vllm".to_owned(),
                attributes: None,
            }),
            agent_config: Some(agent_attrs),
        })
        .await;
    match register_agent {
        Ok(_) => info!(agent_id = AGENT_ID, "RegisterAgent ok (scenario 9)"),
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                agent_id = AGENT_ID,
                "RegisterAgent tolerated (agent already present)"
            );
        }
        Err(status) => bail!("RegisterAgent failed unexpectedly (scenario 9): {status}"),
    }

    let create_council = client
        .create_council(CreateCouncilRequest {
            specialty: SPECIALTY.to_owned(),
            num_agents: 1,
            agent_config: None,
        })
        .await;
    match create_council {
        Ok(_) => info!(specialty = SPECIALTY, "CreateCouncil ok (scenario 9)"),
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                specialty = SPECIALTY,
                "CreateCouncil tolerated (council already present)"
            );
        }
        Err(status) => bail!("CreateCouncil failed unexpectedly (scenario 9): {status}"),
    }

    let nats_url = std::env::var("MADE_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("made.deliberation.completed".to_owned())
        .await
        .context("subscribe made.deliberation.completed for scenario 9")?;
    nats.flush()
        .await
        .context("flush NATS subscribe (scenario 9)")?;

    let response = client
        .run_council_decision(RunCouncilDecisionRequest {
            contract_id: CONTRACT_ID.to_owned(),
            external_context: None,
            validation_mode: ValidationMode::Strict as i32,
            metadata: None,
            description: "scenario 9 structured-output positive path (vllm)".to_owned(),
            selector: Some(RunCouncilSelector::Specialty(SPECIALTY.to_owned())),
        })
        .await
        .context("RunCouncilDecision failed (scenario 9)")?
        .into_inner();

    let winner = response
        .winner
        .ok_or_else(|| anyhow!("RunCouncilDecision returned no winner (scenario 9)"))?;
    let proposal = winner
        .proposal
        .ok_or_else(|| anyhow!("winner has no proposal (scenario 9)"))?;
    let proposal_id = proposal.proposal_id.clone();
    let parsed: serde_json::Value =
        serde_json::from_str(proposal.content.trim()).with_context(|| {
            format!(
                "winner proposal content must parse as JSON (scenario 9); got: {}",
                proposal.content
            )
        })?;
    if let Err(errors) = compiled_schema.validate(&parsed) {
        let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        bail!(
            "winner proposal content failed Report schema (scenario 9): {}",
            messages.join("; ")
        );
    }
    let validation = response
        .validation
        .as_ref()
        .ok_or_else(|| anyhow!("response has no validation summary (scenario 9)"))?;
    if !validation.passed {
        bail!("validation.passed should be true on the positive path (scenario 9)");
    }
    info!(
        proposal_id = proposal_id.as_str(),
        task_id = response.task_id.as_str(),
        candidates_passed = validation.candidates_passed,
        candidates_total = validation.candidates_total,
        "RunCouncilDecision succeeded with Report-shaped winner via vllm kind"
    );

    let task_id = response.task_id.clone();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = remaining.min(Duration::from_secs(2));
        match tokio::time::timeout(chunk, subscription.next()).await {
            Ok(Some(msg)) => {
                let payload: serde_json::Value = serde_json::from_slice(&msg.payload)
                    .context("DeliberationCompleted (scenario 9) payload not JSON")?;
                let got_task = payload.get("task_id").and_then(|v| v.as_str());
                if got_task == Some(task_id.as_str()) {
                    if payload.get("external_context_bundle_id").is_some() {
                        bail!(
                            "scenario 9 envelope must omit external_context_bundle_id (no bundle was sent), got {payload}"
                        );
                    }
                    info!(
                        out_event_id = payload
                            .get("event_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                        task_id = task_id.as_str(),
                        "DeliberationCompleted carried the expected task id (scenario 9)"
                    );
                    return Ok(());
                }
                warn!(
                    ?got_task,
                    "DeliberationCompleted seen for a different task_id; continuing"
                );
            }
            Ok(None) => {
                bail!("NATS subscription closed before scenario 9 envelope arrived");
            }
            Err(_) => {}
        }
    }
    bail!("no DeliberationCompleted for {task_id} arrived within 15s (scenario 9)")
}
