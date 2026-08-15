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

/// Drives `RunCouncilDecision` end-to-end against a stub OpenAI-shaped
/// agent that always emits a JSON Report payload satisfying
/// `api/examples/output-contracts/report.schema.json`. Closes the
/// positive structured-output gap left by scenario 6 (which only
/// proves the rejection path against `NoopAgent`).
///
/// Steps:
///
/// 1. Read the Report JSON Schema from
///    `MADE_REPORT_SCHEMA_PATH` (compose pins it to
///    `/etc/made/report.schema.json`).
/// 2. Register the contract via `RegisterContract`. Tolerate the
///    `AlreadyExists` and `FailedPrecondition` codes so a re-run
///    against the same compose stack does not flake.
/// 3. Register an `openai`-kind agent that points at `http://stub-llm:8000`.
/// 4. Create a council with that single agent under specialty
///    `"report"`.
/// 5. Pre-subscribe to `made.deliberation.completed` so the bus
///    envelope assertion cannot race.
/// 6. Call `RunCouncilDecision` in `STRICT` mode bound to the
///    contract.
/// 7. Assert the response carries a winner whose proposal content
///    parses as JSON and satisfies the Report schema; assert
///    `validation.passed == true`.
/// 8. Assert the outbound envelope carries the same task id and
///    OMITS the `external_context_bundle_id` field (no bundle was
///    passed — the field is `skip_serializing_if = Option::is_none`,
///    so an absence is the contract, not an empty string).
#[allow(clippy::too_many_lines)] // single end-to-end scenario; splitting fragments the assertion
pub(crate) async fn verify_structured_output_against_stub_llm(
    client: &mut MadeServiceClient<Channel>,
) -> Result<()> {
    const CONTRACT_ID: &str = "scenario-8-report";
    // Must match the id pattern the CreateCouncil handler mints
    // (`agent-{specialty}-{i}`); without that pairing the council
    // create step fails resolving the agent.
    const AGENT_ID: &str = "agent-report-0";
    const SPECIALTY: &str = "report";
    const STUB_ENDPOINT: &str = "http://stub-llm:8000";

    let schema_path = std::env::var("MADE_REPORT_SCHEMA_PATH")
        .unwrap_or_else(|_| "/etc/made/report.schema.json".to_owned());
    let schema_body = std::fs::read_to_string(&schema_path)
        .with_context(|| format!("read Report schema at {schema_path}"))?;
    let schema_value: serde_json::Value =
        serde_json::from_str(&schema_body).context("Report schema must parse as JSON")?;
    let compiled_schema = jsonschema::JSONSchema::compile(&schema_value)
        .map_err(|err| anyhow!("Report schema must compile: {err}"))?;

    // 2. Register the contract. Tolerate AlreadyExists /
    // FailedPrecondition so a re-run against the same compose stack
    // does not flake.
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
        Ok(_) => info!(contract_id = CONTRACT_ID, "RegisterContract ok"),
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
        Err(status) => bail!("RegisterContract failed unexpectedly: {status}"),
    }

    // 3. Register the openai-kind agent pointing at the stub-llm
    // sidecar. The attribute keys mirror `DispatchingAgentFactory`'s
    // recognised overrides (`provider.endpoint`, `provider.model`).
    // The api_key attribute is NOT consumed by the factory (which
    // demands a base config from env) — it's carried purely as a
    // forward-compatible marker. compose sets a dummy
    // `MADE_OPENAI_API_KEY` so the factory has a base config.
    let agent_attrs = pb_struct_from_pairs([
        (
            "provider.endpoint",
            PbKind::StringValue(STUB_ENDPOINT.to_owned()),
        ),
        (
            "provider.model",
            PbKind::StringValue("stub-report-v1".to_owned()),
        ),
        (
            "provider.api_key",
            PbKind::StringValue("stub-key-not-used".to_owned()),
        ),
    ]);
    let register_agent = client
        .register_agent(RegisterAgentRequest {
            specialty: SPECIALTY.to_owned(),
            agent: Some(AgentSummary {
                agent_id: AGENT_ID.to_owned(),
                specialty: SPECIALTY.to_owned(),
                kind: "openai".to_owned(),
                // The RegisterAgent mapper reads `agent_config`
                // (NOT this nested attributes field) to feed the
                // descriptor passed to the factory. Carrying the
                // attributes here would silently drop them.
                attributes: None,
            }),
            // `provider.endpoint` + `provider.model` must travel here
            // for the factory to apply them as overrides.
            agent_config: Some(agent_attrs),
        })
        .await;
    match register_agent {
        Ok(_) => info!(agent_id = AGENT_ID, "RegisterAgent ok"),
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                agent_id = AGENT_ID,
                "RegisterAgent tolerated (agent already present)"
            );
        }
        Err(status) => bail!("RegisterAgent failed unexpectedly: {status}"),
    }

    // 4. Create a council under specialty `report`. Tolerate
    // AlreadyExists so a re-run against the same compose stack does
    // not flake.
    let create_council = client
        .create_council(CreateCouncilRequest {
            specialty: SPECIALTY.to_owned(),
            num_agents: 1,
            agent_config: None,
        })
        .await;
    match create_council {
        Ok(_) => info!(specialty = SPECIALTY, "CreateCouncil ok"),
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                specialty = SPECIALTY,
                "CreateCouncil tolerated (council already present)"
            );
        }
        Err(status) => bail!("CreateCouncil failed unexpectedly: {status}"),
    }

    // 5. Pre-subscribe so the published envelope cannot race the
    // RPC.
    let nats_url = std::env::var("MADE_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("made.deliberation.completed".to_owned())
        .await
        .context("subscribe made.deliberation.completed for scenario 8")?;
    nats.flush()
        .await
        .context("flush NATS subscribe (scenario 8)")?;

    // 6. Call RunCouncilDecision.
    let response = client
        .run_council_decision(RunCouncilDecisionRequest {
            contract_id: CONTRACT_ID.to_owned(),
            external_context: None,
            validation_mode: ValidationMode::Strict as i32,
            metadata: None,
            description: "scenario 8 structured-output positive path".to_owned(),
            selector: Some(RunCouncilSelector::Specialty(SPECIALTY.to_owned())),
        })
        .await
        .context("RunCouncilDecision failed (scenario 8)")?
        .into_inner();

    // 7. Assertions on the response.
    let winner = response
        .winner
        .ok_or_else(|| anyhow!("RunCouncilDecision returned no winner"))?;
    let proposal = winner
        .proposal
        .ok_or_else(|| anyhow!("winner has no proposal"))?;
    let proposal_id = proposal.proposal_id.clone();
    let parsed: serde_json::Value =
        serde_json::from_str(proposal.content.trim()).with_context(|| {
            format!(
                "winner proposal content must parse as JSON; got: {}",
                proposal.content
            )
        })?;
    if let Err(errors) = compiled_schema.validate(&parsed) {
        let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        bail!(
            "winner proposal content failed Report schema: {}",
            messages.join("; ")
        );
    }
    let validation = response
        .validation
        .as_ref()
        .ok_or_else(|| anyhow!("response has no validation summary"))?;
    if !validation.passed {
        bail!("validation.passed should be true on the positive path");
    }
    info!(
        proposal_id = proposal_id.as_str(),
        task_id = response.task_id.as_str(),
        candidates_passed = validation.candidates_passed,
        candidates_total = validation.candidates_total,
        "RunCouncilDecision succeeded with Report-shaped winner"
    );

    // 8. Bus envelope assertion. The task did NOT carry an
    // ExternalContextBundle, so the JSON envelope MUST omit the
    // `external_context_bundle_id` field (the field is annotated
    // `skip_serializing_if = Option::is_none`). Asserting absence —
    // not "empty string" — is the contract.
    let task_id = response.task_id.clone();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = remaining.min(Duration::from_secs(2));
        match tokio::time::timeout(chunk, subscription.next()).await {
            Ok(Some(msg)) => {
                let payload: serde_json::Value = serde_json::from_slice(&msg.payload)
                    .context("DeliberationCompleted (scenario 8) payload not JSON")?;
                let got_task = payload.get("task_id").and_then(|v| v.as_str());
                if got_task == Some(task_id.as_str()) {
                    if payload.get("external_context_bundle_id").is_some() {
                        bail!(
                            "scenario 8 envelope must omit external_context_bundle_id (no bundle was sent), got {payload}"
                        );
                    }
                    info!(
                        out_event_id = payload
                            .get("event_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                        task_id = task_id.as_str(),
                        "DeliberationCompleted carried the expected task id and no bundle id"
                    );
                    return Ok(());
                }
                warn!(
                    ?got_task,
                    "DeliberationCompleted seen for a different task_id; continuing"
                );
            }
            Ok(None) => {
                bail!("NATS subscription closed before scenario 8 envelope arrived");
            }
            Err(_) => {}
        }
    }
    bail!("no DeliberationCompleted for {task_id} arrived within 15s (scenario 8)")
}
