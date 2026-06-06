//! Positive path — register a provider-backed agent and require a
//! schema-valid Report winner.
//!
//! This chain is opt-in because it requires the target Choreographer
//! to have a provider kind enabled at boot (`openai` or `vllm`) and
//! the supplied endpoint must speak the OpenAI-compatible
//! `/v1/chat/completions` shape. The default `--chain all` remains the
//! provider-free rejection smoke.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use choreo_proto::v1 as pb;
use choreo_proto::v1::run_council_decision_request::Selector;
use futures::StreamExt;
use prost_types::{value::Kind as PbKind, Struct as PbStruct, Value as PbValue};
use tonic::Code;
use tracing::{debug, info};

use crate::chain2::{validate_payload_against_schema, DEFAULT_REPORT_SCHEMA_PATH};
use crate::outcome::{AssertionRecord, BusEnvelopeRecord, ChainOutcome};
use crate::{Harness, HarnessConfig};

const DELIBERATION_COMPLETED_SUBJECT: &str = "choreo.deliberation.completed";
const BUS_WAIT_BUDGET: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositivePathConfig {
    pub agent_kind: String,
    pub agent_endpoint: Option<String>,
    pub agent_model: String,
    pub specialty: String,
}

impl Default for PositivePathConfig {
    fn default() -> Self {
        Self {
            agent_kind: "openai".to_owned(),
            agent_endpoint: None,
            agent_model: "stub-report-v1".to_owned(),
            specialty: "consumer-smoke-report-openai".to_owned(),
        }
    }
}

/// Run the positive Report path against a connected [`Harness`].
///
/// The function never returns provider or schema mismatches as
/// `Err`; those become typed failed assertions so the CLI can print a
/// useful table and exit `1`. `Err` remains reserved for unexpected
/// infrastructure faults in the harness itself.
#[allow(clippy::too_many_lines)] // one linear consumer transaction is easier to audit than fragments
pub async fn run_positive_path(
    h: &mut Harness,
    cfg: &HarnessConfig,
    positive: &PositivePathConfig,
) -> Result<ChainOutcome> {
    let start = Instant::now();
    let mut assertions = Vec::new();
    let mut bus_envelopes = Vec::new();
    let agent_id = agent_id_for_specialty(&positive.specialty);

    let endpoint = match positive.agent_endpoint.as_deref() {
        Some(endpoint) if !endpoint.trim().is_empty() => {
            assertions.push(AssertionRecord::passed(
                "positive_provider_endpoint_configured",
                Duration::ZERO,
            ));
            endpoint.trim().to_owned()
        }
        _ => {
            assertions.push(AssertionRecord::failed(
                "positive_provider_endpoint_configured",
                "positive path requires --provider-endpoint or CONSUMER_SMOKE_PROVIDER_ENDPOINT",
                Duration::ZERO,
            ));
            assertions.push(AssertionRecord::skipped(
                "report_schema_registered",
                "provider endpoint missing; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_agent_registered",
                "provider endpoint missing; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_council_created",
                "provider endpoint missing; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "run_council_decision_strict",
                "provider endpoint missing; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "provider endpoint missing; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "provider endpoint missing; downstream assertions did not run",
            ));
            return Ok(outcome(
                cfg,
                None,
                None,
                None,
                assertions,
                bus_envelopes,
                start.elapsed(),
            ));
        }
    };

    let schema_path = std::env::var("CHOREO_REPORT_SCHEMA_PATH")
        .unwrap_or_else(|_| DEFAULT_REPORT_SCHEMA_PATH.to_owned());
    let schema_body = match std::fs::read_to_string(&schema_path) {
        Ok(schema) => schema,
        Err(err) => {
            assertions.push(AssertionRecord::failed(
                "report_schema_registered",
                format!("schema not found at {schema_path}: {err}"),
                Duration::ZERO,
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_agent_registered",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_council_created",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "run_council_decision_strict",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "schema-registration step failed; downstream assertions did not run",
            ));
            return Ok(outcome(
                cfg,
                None,
                None,
                None,
                assertions,
                bus_envelopes,
                start.elapsed(),
            ));
        }
    };

    let register_start = Instant::now();
    let register_contract = h
        .grpc
        .register_contract(pb::RegisterContractRequest {
            contract: Some(pb::OutputContract {
                contract_id: cfg.contract_id.clone(),
                format: pb::OutputFormat::JsonObject as i32,
                fields: HashMap::new(),
                json_schema: schema_body.clone(),
            }),
        })
        .await;
    let register_elapsed = register_start.elapsed();
    match register_contract {
        Ok(_) => assertions.push(AssertionRecord::passed(
            "report_schema_registered",
            register_elapsed,
        )),
        Err(status)
            if matches!(
                status.code(),
                Code::AlreadyExists | Code::FailedPrecondition
            ) =>
        {
            assertions.push(AssertionRecord::passed(
                "report_schema_registered",
                register_elapsed,
            ));
        }
        Err(status) => {
            assertions.push(AssertionRecord::failed(
                "report_schema_registered",
                format!("RegisterContract returned {status:?}"),
                register_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_agent_registered",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_council_created",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "run_council_decision_strict",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "schema-registration step failed; downstream assertions did not run",
            ));
            return Ok(outcome(
                cfg,
                None,
                None,
                None,
                assertions,
                bus_envelopes,
                start.elapsed(),
            ));
        }
    }

    let register_agent_start = Instant::now();
    let register_agent = h
        .grpc
        .register_agent(pb::RegisterAgentRequest {
            specialty: positive.specialty.clone(),
            agent: Some(pb::AgentSummary {
                agent_id: agent_id.clone(),
                specialty: positive.specialty.clone(),
                kind: positive.agent_kind.clone(),
                attributes: None,
            }),
            agent_config: Some(provider_attrs(&endpoint, &positive.agent_model)),
        })
        .await;
    let register_agent_elapsed = register_agent_start.elapsed();
    match register_agent {
        Ok(_) => assertions.push(AssertionRecord::passed(
            "positive_agent_registered",
            register_agent_elapsed,
        )),
        Err(status) if status.code() == Code::AlreadyExists => assertions.push(
            AssertionRecord::passed("positive_agent_registered", register_agent_elapsed),
        ),
        Err(status) => {
            assertions.push(AssertionRecord::failed(
                "positive_agent_registered",
                format!("RegisterAgent returned {status:?}"),
                register_agent_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "positive_council_created",
                "agent-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "run_council_decision_strict",
                "agent-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "agent-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "agent-registration step failed; downstream assertions did not run",
            ));
            return Ok(outcome(
                cfg,
                None,
                None,
                None,
                assertions,
                bus_envelopes,
                start.elapsed(),
            ));
        }
    }

    let create_start = Instant::now();
    let create_council = h
        .grpc
        .create_council(pb::CreateCouncilRequest {
            specialty: positive.specialty.clone(),
            num_agents: 1,
            agent_config: None,
        })
        .await;
    let create_elapsed = create_start.elapsed();
    match create_council {
        Ok(_) => assertions.push(AssertionRecord::passed(
            "positive_council_created",
            create_elapsed,
        )),
        Err(status) if status.code() == Code::AlreadyExists => assertions.push(
            AssertionRecord::passed("positive_council_created", create_elapsed),
        ),
        Err(status) => {
            assertions.push(AssertionRecord::failed(
                "positive_council_created",
                format!("CreateCouncil returned {status:?}"),
                create_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "run_council_decision_strict",
                "council-create step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "council-create step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "council-create step failed; downstream assertions did not run",
            ));
            return Ok(outcome(
                cfg,
                None,
                None,
                None,
                assertions,
                bus_envelopes,
                start.elapsed(),
            ));
        }
    }

    let mut subscription = match h.nats.as_ref() {
        Some(client) => match client.subscribe(DELIBERATION_COMPLETED_SUBJECT).await {
            Ok(sub) => {
                if let Err(err) = client.flush().await {
                    debug!(error = %err, "flush after positive-path subscribe failed");
                }
                Some(sub)
            }
            Err(err) => {
                debug!(error = %err, "positive-path subscribe failed");
                None
            }
        },
        None => None,
    };

    let rpc_start = Instant::now();
    let response = h
        .grpc
        .run_council_decision(pb::RunCouncilDecisionRequest {
            contract_id: cfg.contract_id.clone(),
            external_context: None,
            validation_mode: pb::ValidationMode::Strict as i32,
            metadata: None,
            description: "consumer-smoke positive Report path".to_owned(),
            selector: Some(Selector::Specialty(positive.specialty.clone())),
        })
        .await;
    let rpc_elapsed = rpc_start.elapsed();

    let (task_id, winner_proposal_id, validation_passed) = match response {
        Ok(resp) => {
            let resp = resp.into_inner();
            let task_id = if resp.task_id.is_empty() {
                None
            } else {
                Some(resp.task_id.clone())
            };
            let winner_proposal_id = resp
                .winner
                .as_ref()
                .and_then(|d| d.proposal.as_ref())
                .map(|p| p.proposal_id.clone());
            let validation_passed = resp.validation.as_ref().map(|v| v.passed);

            let content = resp
                .winner
                .as_ref()
                .and_then(|d| d.proposal.as_ref())
                .map(|p| p.content.clone());
            if content.is_some() {
                assertions.push(AssertionRecord::passed(
                    "run_council_decision_strict",
                    rpc_elapsed,
                ));
            } else {
                assertions.push(AssertionRecord::failed(
                    "run_council_decision_strict",
                    "RunCouncilDecision returned no winner proposal",
                    rpc_elapsed,
                ));
            }
            assertions.push(validate_payload_against_schema(
                &schema_body,
                content.as_deref().unwrap_or_default(),
                rpc_elapsed,
            ));
            match resp.validation.as_ref() {
                Some(v) if v.passed => assertions.push(AssertionRecord::passed(
                    "report_validation_summary_passed",
                    Duration::ZERO,
                )),
                Some(v) => assertions.push(AssertionRecord::failed(
                    "report_validation_summary_passed",
                    format!(
                        "validation.passed=false candidates_passed={} candidates_total={}",
                        v.candidates_passed, v.candidates_total
                    ),
                    Duration::ZERO,
                )),
                None => assertions.push(AssertionRecord::failed(
                    "report_validation_summary_passed",
                    "response has no validation summary",
                    Duration::ZERO,
                )),
            }
            (task_id, winner_proposal_id, validation_passed)
        }
        Err(status) => {
            let detail = format!("{status:?}");
            assertions.push(AssertionRecord::failed(
                "run_council_decision_strict",
                detail.clone(),
                rpc_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "RunCouncilDecision failed; no payload to validate",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_validation_summary_passed",
                "RunCouncilDecision failed; no validation summary returned",
            ));
            (None, None, None)
        }
    };

    match (subscription.as_mut(), task_id.as_deref()) {
        (None, _) => assertions.push(AssertionRecord::skipped(
            "positive_completion_envelope_observed",
            "no NATS configured; bus-coupled assertion disabled",
        )),
        (Some(_), None) => assertions.push(AssertionRecord::skipped(
            "positive_completion_envelope_observed",
            "RunCouncilDecision did not return task_id",
        )),
        (Some(sub), Some(task_id)) => {
            let wait_start = Instant::now();
            let matched = wait_for_completion(sub, task_id, &mut bus_envelopes).await;
            let wait_elapsed = wait_start.elapsed();
            if matched {
                assertions.push(AssertionRecord::passed(
                    "positive_completion_envelope_observed",
                    wait_elapsed,
                ));
            } else {
                assertions.push(AssertionRecord::failed(
                    "positive_completion_envelope_observed",
                    format!(
                        "no deliberation.completed envelope for task_id={task_id:?} \
                         observed within {BUS_WAIT_BUDGET:?}"
                    ),
                    wait_elapsed,
                ));
            }
        }
    }

    let outcome = outcome(
        cfg,
        task_id,
        winner_proposal_id,
        validation_passed,
        assertions,
        bus_envelopes,
        start.elapsed(),
    );
    info!(
        chain = outcome.chain,
        passed = outcome.passed(),
        passed_count = outcome.passed_count(),
        failed_count = outcome.failed_count(),
        skipped_count = outcome.skipped_count(),
        provider_kind = positive.agent_kind,
        specialty = positive.specialty,
        "consumer-smoke positive path finished"
    );
    Ok(outcome)
}

fn outcome(
    cfg: &HarnessConfig,
    task_id: Option<String>,
    winner_proposal_id: Option<String>,
    validation_passed: Option<bool>,
    assertions: Vec<AssertionRecord>,
    bus_envelopes: Vec<BusEnvelopeRecord>,
    total_duration: Duration,
) -> ChainOutcome {
    ChainOutcome {
        chain: "positive-path",
        contract_id: cfg.contract_id.clone(),
        task_id,
        winner_proposal_id,
        validation_passed,
        assertions,
        bus_envelopes,
        total_duration,
    }
}

fn provider_attrs(endpoint: &str, model: &str) -> PbStruct {
    PbStruct {
        fields: [
            (
                "provider.endpoint".to_owned(),
                PbValue {
                    kind: Some(PbKind::StringValue(endpoint.to_owned())),
                },
            ),
            (
                "provider.model".to_owned(),
                PbValue {
                    kind: Some(PbKind::StringValue(model.to_owned())),
                },
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn agent_id_for_specialty(specialty: &str) -> String {
    format!("agent-{specialty}-0")
}

async fn wait_for_completion(
    subscription: &mut async_nats::Subscriber,
    task_id: &str,
    bus_envelopes: &mut Vec<BusEnvelopeRecord>,
) -> bool {
    let deadline = Instant::now() + BUS_WAIT_BUDGET;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let next = tokio::time::timeout(remaining, subscription.next()).await;
        let Ok(Some(msg)) = next else {
            break;
        };
        let value: serde_json::Value = match serde_json::from_slice(&msg.payload) {
            Ok(v) => v,
            Err(err) => {
                debug!(error = %err, "non-JSON deliberation.completed payload; skipping");
                continue;
            }
        };
        let envelope_record = BusEnvelopeRecord {
            subject: msg.subject.to_string(),
            event_id: value
                .get("event_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            correlation_id: value
                .get("correlation_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            causation_id: value
                .get("causation_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        };
        bus_envelopes.push(envelope_record);
        if value.get("task_id").and_then(serde_json::Value::as_str) == Some(task_id) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_positive_path_config_is_openai_shaped() {
        let cfg = PositivePathConfig::default();
        assert_eq!(cfg.agent_kind, "openai");
        assert_eq!(cfg.agent_model, "stub-report-v1");
        assert_eq!(cfg.specialty, "consumer-smoke-report-openai");
        assert!(cfg.agent_endpoint.is_none());
    }

    #[test]
    fn agent_id_matches_create_council_convention() {
        assert_eq!(
            agent_id_for_specialty("consumer-smoke-report-openai"),
            "agent-consumer-smoke-report-openai-0"
        );
    }

    #[test]
    fn provider_attrs_carry_endpoint_and_model() {
        let attrs = provider_attrs("http://stub-llm:8000", "stub-report-v1");
        assert_eq!(attrs.fields.len(), 2);
        assert!(attrs.fields.contains_key("provider.endpoint"));
        assert!(attrs.fields.contains_key("provider.model"));
    }
}
