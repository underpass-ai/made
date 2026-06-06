use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::run_council_decision_request::Selector as RunCouncilSelector;
use choreo_proto::v1::{
    AgentSummary, CreateCouncilRequest, OutputContract, OutputFormat, RegisterAgentRequest,
    RegisterContractRequest, RunCouncilDecisionRequest, ValidationMode,
};
use prost_types::value::Kind as PbKind;
use tonic::transport::Channel;
use tonic::Code;
use tracing::info;

use super::super::pb_struct_from_pairs;

const VLLM_ENDPOINT_ENV: &str = "CHOREO_VLLM_ENDPOINT";
const VLLM_MODEL_ENV: &str = "CHOREO_VLLM_MODEL";
const VLLM_AGENT_COUNT_ENV: &str = "CHOREO_VLLM_AGENT_COUNT";
const VLLM_MAX_TOKENS_ENV: &str = "CHOREO_VLLM_MAX_TOKENS";
const VLLM_TIMEOUT_SECS_ENV: &str = "CHOREO_VLLM_TIMEOUT_SECS";
const RUN_ID_ENV: &str = "CHOREO_E2E_RUN_ID";

/// Full Choreographer E2E against a real vLLM provider.
///
/// This differs from the provider-level runner: it registers multiple
/// `kind=vllm` agents through the public gRPC surface, creates a real
/// council, then runs `RunCouncilDecision` in Strict mode and asserts
/// that more than one candidate was considered and revised by the
/// peer-review ceremony.
#[allow(clippy::too_many_lines)] // single real-provider E2E scenario; splitting fragments the assertion
pub(crate) async fn verify_multi_agent_council_against_real_vllm(
    client: &mut ChoreographerServiceClient<Channel>,
) -> Result<()> {
    let endpoint = required_env(VLLM_ENDPOINT_ENV)?;
    let model = required_env(VLLM_MODEL_ENV)?;
    let agent_count = env_u32(VLLM_AGENT_COUNT_ENV, 3)?;
    let max_tokens = env_u32(VLLM_MAX_TOKENS_ENV, 512)?;
    let timeout_secs = env_u32(VLLM_TIMEOUT_SECS_ENV, 300)?;
    if agent_count < 2 {
        bail!("{VLLM_AGENT_COUNT_ENV} must be >= 2 for the multi-agent vLLM E2E");
    }
    let run_id = std::env::var(RUN_ID_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(default_run_id);
    let specialty = format!("report-vllm-real-{run_id}");
    let contract_id = format!("scenario-10-report-vllm-real-{run_id}");

    let schema_body = e2e_report_schema();
    let schema_value: serde_json::Value =
        serde_json::from_str(&schema_body).context("Report schema must parse as JSON")?;
    let compiled_schema = jsonschema::JSONSchema::compile(&schema_value)
        .map_err(|err| anyhow!("Report schema must compile: {err}"))?;

    register_contract(client, &contract_id, &schema_body).await?;

    for i in 0..agent_count {
        let agent_id = format!("agent-{specialty}-{i}");
        register_vllm_agent(
            client,
            &agent_id,
            &specialty,
            &endpoint,
            &model,
            max_tokens,
            timeout_secs,
        )
        .await?;
    }

    create_council(client, &specialty, agent_count).await?;

    let response = client
        .run_council_decision(RunCouncilDecisionRequest {
            contract_id: contract_id.clone(),
            external_context: None,
            validation_mode: ValidationMode::Strict as i32,
            metadata: None,
            description: format!(
                "E2E run {run_id}: produce a concise incident-style Report JSON. \
                 Return only a JSON object, with no markdown and no explanatory text. \
                 Use exactly this minimal shape: \
                 {{\"report_id\":\"{run_id}\",\"executive_summary\":\"one concise paragraph\",\
                 \"findings\":[{{\"id\":\"finding-1\",\"summary\":\"one concrete finding\",\
                 \"confidence\":\"medium\"}}],\"recommended_actions\":[{{\"id\":\"action-1\",\
                 \"summary\":\"one concrete action\",\"approval_required\":false}}]}}. \
                 You may change string values to fit the task, but do not add top-level fields \
                 and do not add object fields that are not present in the JSON Schema."
            ),
            selector: Some(RunCouncilSelector::Specialty(specialty.clone())),
        })
        .await
        .with_context(|| {
            format!("RunCouncilDecision failed for multi-agent real vLLM council `{specialty}`")
        })?
        .into_inner();

    let winner = response
        .winner
        .ok_or_else(|| anyhow!("RunCouncilDecision returned no winner"))?;
    let proposal = winner
        .proposal
        .ok_or_else(|| anyhow!("winner has no proposal"))?;
    let validation = response
        .validation
        .as_ref()
        .ok_or_else(|| anyhow!("response has no validation summary"))?;
    if !validation.passed {
        bail!(
            "expected the Strict-mode vLLM council decision to pass the Report contract; candidate reports: {}",
            summarize_candidate_reports(&response.candidates)
        );
    }
    if proposal.revision_count == 0 {
        bail!(
            "winner proposal was not revised; expected peer-review interaction before validation"
        );
    }
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

    if validation.candidates_total < agent_count {
        bail!(
            "expected at least {agent_count} candidates from the vLLM council, got {}",
            validation.candidates_total
        );
    }
    if validation.candidates_passed == 0 {
        bail!("expected at least one schema-valid candidate from the vLLM council");
    }
    let unrevised: Vec<String> = response
        .candidates
        .iter()
        .filter(|candidate| candidate.revision_count == 0)
        .map(|candidate| candidate.proposal_id.clone())
        .collect();
    if !unrevised.is_empty() {
        bail!(
            "expected every vLLM candidate to have peer-review revisions; unrevised proposal ids: {}",
            unrevised.join(", ")
        );
    }
    let distinct_authors = response
        .candidates
        .iter()
        .map(|candidate| candidate.author_agent_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if distinct_authors.len() < agent_count as usize {
        bail!(
            "expected candidates from {agent_count} distinct vLLM agents, got {}",
            distinct_authors.len()
        );
    }

    info!(
        task_id = response.task_id.as_str(),
        specialty,
        contract_id,
        endpoint,
        model,
        agent_count,
        max_tokens,
        timeout_secs,
        winner_revision_count = proposal.revision_count,
        candidates_passed = validation.candidates_passed,
        candidates_total = validation.candidates_total,
        "multi-agent real vLLM peer-review ceremony produced a schema-valid winner"
    );
    Ok(())
}

fn e2e_report_schema() -> String {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://underpass.ai/choreographer/e2e/report-minimal.schema.json",
        "title": "Minimal Report Output Contract for real vLLM E2E",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "report_id",
            "executive_summary",
            "findings",
            "recommended_actions"
        ],
        "properties": {
            "report_id": {
                "type": "string",
                "minLength": 1,
                "maxLength": 128
            },
            "executive_summary": {
                "type": "string",
                "minLength": 1,
                "maxLength": 1024
            },
            "findings": {
                "type": "array",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "summary", "confidence"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 64
                        },
                        "summary": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 512
                        },
                        "confidence": {
                            "type": "string",
                            "enum": ["high", "medium", "low", "unknown"]
                        }
                    }
                }
            },
            "recommended_actions": {
                "type": "array",
                "minItems": 1,
                "maxItems": 3,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "summary", "approval_required"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 64
                        },
                        "summary": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 512
                        },
                        "approval_required": {
                            "type": "boolean"
                        }
                    }
                }
            }
        }
    })
    .to_string()
}

fn summarize_candidate_reports(candidates: &[choreo_proto::v1::CandidateSummary]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            let reports = candidate
                .reports
                .iter()
                .map(|report| {
                    format!(
                        "{}:{}:{}",
                        report.kind,
                        if report.passed { "passed" } else { "failed" },
                        report.summary
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            format!(
                "{} rank={} score={} revised={} reports=[{}]",
                candidate.proposal_id,
                candidate.rank,
                candidate.score,
                candidate.revision_count,
                reports
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

async fn register_contract(
    client: &mut ChoreographerServiceClient<Channel>,
    contract_id: &str,
    schema_body: &str,
) -> Result<()> {
    let result = client
        .register_contract(RegisterContractRequest {
            contract: Some(OutputContract {
                contract_id: contract_id.to_owned(),
                format: OutputFormat::JsonObject as i32,
                fields: std::collections::HashMap::new(),
                json_schema: schema_body.to_owned(),
            }),
        })
        .await;
    match result {
        Ok(_) => {
            info!(contract_id, "RegisterContract ok (real vLLM multi-agent)");
            Ok(())
        }
        Err(status)
            if matches!(
                status.code(),
                Code::AlreadyExists | Code::FailedPrecondition
            ) =>
        {
            info!(
                code = ?status.code(),
                contract_id,
                "RegisterContract tolerated (contract already present)"
            );
            Ok(())
        }
        Err(status) => bail!("RegisterContract failed unexpectedly: {status}"),
    }
}

async fn register_vllm_agent(
    client: &mut ChoreographerServiceClient<Channel>,
    agent_id: &str,
    specialty: &str,
    endpoint: &str,
    model: &str,
    max_tokens: u32,
    timeout_secs: u32,
) -> Result<()> {
    let attrs = pb_struct_from_pairs([
        (
            "provider.endpoint",
            PbKind::StringValue(endpoint.to_owned()),
        ),
        ("provider.model", PbKind::StringValue(model.to_owned())),
        (
            "provider.max_tokens",
            PbKind::NumberValue(max_tokens.into()),
        ),
        (
            "provider.timeout_secs",
            PbKind::NumberValue(timeout_secs.into()),
        ),
    ]);
    let result = client
        .register_agent(RegisterAgentRequest {
            specialty: specialty.to_owned(),
            agent: Some(AgentSummary {
                agent_id: agent_id.to_owned(),
                specialty: specialty.to_owned(),
                kind: "vllm".to_owned(),
                attributes: None,
            }),
            agent_config: Some(attrs),
        })
        .await;
    match result {
        Ok(_) => {
            info!(agent_id, specialty, "RegisterAgent ok (real vLLM)");
            Ok(())
        }
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                agent_id,
                specialty, "RegisterAgent tolerated (agent already present)"
            );
            Ok(())
        }
        Err(status) => bail!("RegisterAgent failed unexpectedly for {agent_id}: {status}"),
    }
}

async fn create_council(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
    agent_count: u32,
) -> Result<()> {
    let result = client
        .create_council(CreateCouncilRequest {
            specialty: specialty.to_owned(),
            num_agents: agent_count,
            agent_config: None,
        })
        .await;
    match result {
        Ok(_) => {
            info!(specialty, agent_count, "CreateCouncil ok (real vLLM)");
            Ok(())
        }
        Err(status) if status.code() == Code::AlreadyExists => {
            info!(
                specialty,
                "CreateCouncil tolerated (council already present)"
            );
            Ok(())
        }
        Err(status) => bail!("CreateCouncil failed unexpectedly: {status}"),
    }
}

fn required_env(name: &str) -> Result<String> {
    let raw = std::env::var(name).map_err(|_| anyhow!("required env var {name} is not set"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("required env var {name} is empty");
    }
    Ok(trimmed.to_owned())
}

fn env_u32(name: &str, default: u32) -> Result<u32> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|raw| {
            raw.parse::<u32>()
                .with_context(|| format!("{name} must parse as u32"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn default_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    format!("{}-{millis}", std::process::id())
}
