//! End-to-end runner.
//!
//! Connects to a running Choreographer over gRPC and executes a
//! sequence of scenarios that only pass if the entire stack
//! (bin + in-memory adapters + gRPC surface + seeding) is wired
//! correctly. Intended to run either inside the docker-compose stack
//! or as a Kubernetes Job against a Helm-installed release.
//!
//! Exits 0 on success, non-zero on the first failed assertion.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::{
    Constraints, DeleteCouncilRequest, DeliberateRequest, ExternalContextBundle,
    ListCouncilsRequest, OrchestrateRequest, OutputContract, OutputFormat, Task,
};
use futures::StreamExt;
use prost_types::{value::Kind as PbKind, Struct as PbStruct, Value as PbValue};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tonic::transport::{Channel, Endpoint};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
#[allow(clippy::too_many_lines)] // linear narrative of seven scenarios; splitting fragments the story
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

    let mut client = connect_with_retry(&endpoint, Duration::from_secs(30)).await?;

    info!("scenario 1: seeded council is visible");
    let councils = client
        .list_councils(ListCouncilsRequest {
            include_agents: false,
        })
        .await
        .context("ListCouncils failed")?
        .into_inner()
        .councils;
    if councils.is_empty() {
        bail!(
            "expected at least one seeded council — did the choreographer start with CHOREO_SEED_SPECIALTIES?"
        );
    }
    if !councils.iter().any(|c| c.specialty == seed_specialty) {
        bail!(
            "seeded specialty {seed_specialty} not found among {:?}",
            councils.iter().map(|c| &c.specialty).collect::<Vec<_>>()
        );
    }

    info!("scenario 2: Deliberate on the seeded specialty returns a winner");
    let response = client
        .deliberate(DeliberateRequest {
            task: Some(Task {
                task_id: "e2e-task-1".to_owned(),
                specialty: seed_specialty.clone(),
                description: "End-to-end test: describe the situation.".to_owned(),
                constraints: None,
                attributes: None,
                external_context: None,
                metadata: None,
            }),
        })
        .await
        .context("Deliberate failed")?
        .into_inner();

    if response.task_id != "e2e-task-1" {
        bail!("response.task_id = {:?}", response.task_id);
    }
    if response.winner_proposal_id.is_empty() {
        bail!("winner_proposal_id is empty");
    }
    if response.results.is_empty() {
        bail!("results[] is empty");
    }
    let winner = response
        .results
        .iter()
        .find(|r| r.rank == 0)
        .ok_or_else(|| anyhow!("no result with rank=0"))?;
    let winner_id = winner
        .proposal
        .as_ref()
        .map(|p| p.proposal_id.clone())
        .ok_or_else(|| anyhow!("rank=0 result has no proposal"))?;
    if winner_id != response.winner_proposal_id {
        bail!(
            "rank=0 proposal id {} disagrees with winner_proposal_id {}",
            winner_id,
            response.winner_proposal_id
        );
    }

    info!("scenario 3: DeleteCouncil on a missing specialty returns deleted=false");
    let delete = client
        .delete_council(DeleteCouncilRequest {
            specialty: "unknown-specialty".to_owned(),
        })
        .await
        .context("DeleteCouncil(unknown) failed")?
        .into_inner();
    if delete.deleted {
        bail!("DeleteCouncil on an unknown specialty must return deleted=false");
    }

    info!("scenario 4: causal metadata propagates from inbound trigger to outbound bus event");
    verify_causal_metadata_propagates_over_nats(&seed_specialty)
        .await
        .context("scenario 4 failed")?;

    info!("scenario 5: Orchestrate routes the winner through the configured Runtime executor");
    verify_orchestrate_invokes_runtime_executor(&mut client, &seed_specialty)
        .await
        .context("scenario 5 failed")?;

    info!("scenario 6: Orchestrate with a strict JSON Schema output contract rejects free-form proposals");
    verify_orchestrate_rejects_proposal_violating_json_schema(&mut client, &seed_specialty)
        .await
        .context("scenario 6 failed")?;

    info!(
        "scenario 7: ExternalContextBundle round-trips to the outbound DeliberationCompleted envelope"
    );
    verify_external_context_bundle_round_trips(&mut client, &seed_specialty)
        .await
        .context("scenario 7 failed")?;

    info!("E2E scenarios passed");
    Ok(())
}

/// Drives `Orchestrate` on the seeded council with a task that carries
/// a `runtime.tool_name` attribute. The compose stack wires
/// `CHOREO_EXECUTOR_KIND=runtime` pointing at the in-stack
/// `stub-runtime` sidecar, so a successful outcome here proves the
/// full chain — Deliberate -> winner -> RuntimeExecutor -> gRPC to
/// `underpass.runtime.v1.{SessionService,InvocationService}` -> back
/// up the use-case -> `TaskCompleted`.
async fn verify_orchestrate_invokes_runtime_executor(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    let attributes = pb_struct_from_pairs([(
        "runtime.tool_name",
        PbKind::StringValue("stub.echo".to_owned()),
    )]);

    let response = client
        .orchestrate(OrchestrateRequest {
            task: Some(Task {
                task_id: "e2e-task-5".to_owned(),
                specialty: specialty.to_owned(),
                description: "End-to-end test: route winner through the Runtime executor."
                    .to_owned(),
                // No output_contract: NoopAgent emits free-form text,
                // and a structured-output contract would force the
                // proposal to be a JSON object. Validators stay no-op
                // without a contract.
                constraints: None,
                attributes: Some(attributes),
                external_context: None,
                metadata: None,
            }),
            execution_options: None,
        })
        .await
        .context("Orchestrate failed")?
        .into_inner();

    if response.task_id != "e2e-task-5" {
        bail!("response.task_id = {:?}", response.task_id);
    }
    if response.execution_id != "stub-invocation-1" {
        bail!(
            "execution_id should be the stub-runtime's canned value, got {:?}",
            response.execution_id
        );
    }
    let winner = response
        .winner
        .ok_or_else(|| anyhow!("Orchestrate returned no winner"))?;
    let winner_proposal = winner
        .proposal
        .ok_or_else(|| anyhow!("winner has no proposal"))?;
    if winner_proposal.content.trim().is_empty() {
        bail!("winner proposal content is empty");
    }
    info!(
        execution_id = response.execution_id.as_str(),
        winner_id = winner_proposal.proposal_id.as_str(),
        "Orchestrate routed through stub-runtime"
    );
    Ok(())
}

fn pb_struct_from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, PbKind)>) -> PbStruct {
    PbStruct {
        fields: pairs
            .into_iter()
            .map(|(k, kind)| (k.to_owned(), PbValue { kind: Some(kind) }))
            .collect(),
    }
}

/// Drives `Orchestrate` on the seeded council with a strict
/// JSON-Schema output contract. `NoopAgent` emits free-form text, so
/// the proposal cannot satisfy `{"type": "object", "required": [...]}`
/// — the deliberation must fail with `NoValidProposal` and the
/// orchestrator must publish a `TaskFailed` event whose envelope
/// carries `error_kind = "deliberation.no_valid_proposal"`.
///
/// This is the negative half of Epic 11's structured-output stack
/// proof. A positive scenario requires an agent that emits structured
/// JSON; that lands once a stub-LLM provider sidecar ships, or once a
/// real provider council is wired into this compose stack.
#[allow(clippy::too_many_lines)] // single end-to-end scenario; splitting fragments the assertion
async fn verify_orchestrate_rejects_proposal_violating_json_schema(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    // Pre-subscribe so the published TaskFailed envelope cannot land
    // before we are watching for it.
    let nats_url =
        std::env::var("CHOREO_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("choreo.task.failed".to_owned())
        .await
        .context("subscribe choreo.task.failed")?;
    nats.flush().await.context("flush NATS subscribe")?;

    let attributes = pb_struct_from_pairs([(
        "runtime.tool_name",
        PbKind::StringValue("stub.echo".to_owned()),
    )]);
    let constraints = Constraints {
        rubric: None,
        rounds: 0,
        num_agents: 0,
        deadline_ms: 0,
        output_contract: Some(OutputContract {
            contract_id: "scenario-6-strict".to_owned(),
            format: OutputFormat::JsonObject as i32,
            fields: std::collections::HashMap::new(),
            json_schema: r#"{
                "type": "object",
                "additionalProperties": false,
                "required": ["decision", "reason"],
                "properties": {
                    "decision": { "type": "string", "enum": ["emit_event", "escalate"] },
                    "reason":   { "type": "string", "minLength": 1 }
                }
            }"#
            .to_owned(),
        }),
    };

    let result = client
        .orchestrate(OrchestrateRequest {
            task: Some(Task {
                task_id: "e2e-task-6".to_owned(),
                specialty: specialty.to_owned(),
                description:
                    "End-to-end test: strict structured-output contract must reject free-form text."
                        .to_owned(),
                constraints: Some(constraints),
                attributes: Some(attributes),
                external_context: None,
                metadata: None,
            }),
            execution_options: None,
        })
        .await;

    let Err(status) = result else {
        bail!(
            "Orchestrate unexpectedly succeeded — NoopAgent should have failed the JSON Schema contract"
        );
    };
    info!(
        code = ?status.code(),
        message = status.message(),
        "Orchestrate returned the expected error"
    );

    // The use case publishes `TaskFailed` BEFORE returning the error,
    // so the envelope should already be on the bus.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = remaining.min(Duration::from_secs(2));
        match tokio::time::timeout(chunk, subscription.next()).await {
            Ok(Some(msg)) => {
                let payload: serde_json::Value =
                    serde_json::from_slice(&msg.payload).context("task.failed payload not JSON")?;
                let task_id = payload
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let error_kind = payload
                    .get("error_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if task_id == "e2e-task-6" && error_kind == "deliberation.no_valid_proposal" {
                    info!(
                        out_event_id = payload
                            .get("event_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                        error_kind, "TaskFailed carried the expected NoValidProposal kind"
                    );
                    return Ok(());
                }
                warn!(
                    task_id,
                    error_kind,
                    "TaskFailed seen but task_id / error_kind did not match; continuing"
                );
            }
            Ok(None) => {
                bail!("NATS subscription closed before a matching TaskFailed envelope arrived");
            }
            Err(_) => {}
        }
    }

    bail!(
        "no TaskFailed envelope with task_id=e2e-task-6 and error_kind=deliberation.no_valid_proposal arrived within 10s"
    )
}

/// Publishes a `TriggerEvent` on NATS with known `correlation_id` and
/// `causation_id`, then asserts that the resulting
/// `DeliberationCompleted` envelope on the outbound bus carries the
/// same causal ids. Closes the stack-E2E loose end of Epic 5.
async fn verify_causal_metadata_propagates_over_nats(specialty: &str) -> Result<()> {
    let nats_url =
        std::env::var("CHOREO_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let client = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    info!(nats_url, "connected to NATS for causal metadata assertion");

    let mut subscription = client
        .subscribe("choreo.deliberation.completed".to_owned())
        .await
        .context("subscribe choreo.deliberation.completed")?;
    // Flush so the SUB is acked by the server before we publish the
    // trigger that should fan out into the event we're waiting for.
    client.flush().await.context("flush NATS subscribe")?;

    let event_id = "stack-e2e-trigger-1";
    let correlation_id = "stack-e2e-corr";
    let causation_id = "stack-e2e-cause";
    let emitted_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("format emitted_at as RFC 3339")?;
    let trigger = serde_json::json!({
        "event_id": event_id,
        "emitted_at": emitted_at,
        "source": "stack-e2e-runner",
        "correlation_id": correlation_id,
        "causation_id": causation_id,
        "kind": "stack.e2e.trigger",
        "requested_specialties": [specialty],
    });
    let subject = format!("choreo.trigger.{specialty}");
    let payload = serde_json::to_vec(&trigger).context("serialize trigger payload")?;
    client
        .publish(subject.clone(), payload.into())
        .await
        .with_context(|| format!("publish trigger to {subject}"))?;
    client.flush().await.context("flush NATS publish")?;
    info!(subject, correlation_id, causation_id, "trigger published");

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = remaining.min(Duration::from_secs(2));
        match tokio::time::timeout(chunk, subscription.next()).await {
            Ok(Some(msg)) => {
                let payload: serde_json::Value = serde_json::from_slice(&msg.payload)
                    .context("DeliberationCompleted payload not JSON")?;
                let got_corr = payload.get("correlation_id").and_then(|v| v.as_str());
                let got_cause = payload.get("causation_id").and_then(|v| v.as_str());
                if got_corr == Some(correlation_id) && got_cause == Some(causation_id) {
                    info!(
                        out_event_id = payload
                            .get("event_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                        correlation_id,
                        causation_id,
                        "DeliberationCompleted carried our causal ids"
                    );
                    return Ok(());
                }
                warn!(
                    ?got_corr,
                    ?got_cause,
                    "DeliberationCompleted seen but causal ids did not match; continuing"
                );
            }
            Ok(None) => {
                bail!("NATS subscription closed before a matching DeliberationCompleted arrived");
            }
            Err(_) => {}
        }
    }

    bail!("no DeliberationCompleted with the expected causal ids arrived within 15s")
}

async fn connect_with_retry(
    endpoint: &str,
    total_budget: Duration,
) -> Result<ChoreographerServiceClient<Channel>> {
    let deadline = std::time::Instant::now() + total_budget;
    let endpoint_parsed: Endpoint = endpoint
        .parse()
        .with_context(|| format!("invalid gRPC endpoint: {endpoint}"))?;

    let mut last_err: Option<tonic::transport::Error> = None;
    while std::time::Instant::now() < deadline {
        match ChoreographerServiceClient::connect(endpoint_parsed.clone()).await {
            Ok(c) => {
                info!(endpoint, "connected");
                return Ok(c);
            }
            Err(err) => {
                warn!(endpoint, error = %err, "not ready yet; will retry");
                last_err = Some(err);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err(anyhow!(
        "could not connect to {endpoint} within {total_budget:?}: {last_err:?}"
    ))
}

/// Drives `Deliberate` with an `ExternalContextBundle` attached and
/// asserts that the outbound `choreo.deliberation.completed` envelope
/// carries the bundle id back. Closes the round-trip gap that
/// scenario 4 leaves open (scenario 4 covers causal ids; this one
/// covers the bundle pointer the kernel-shaped consumer would feed
/// in).
async fn verify_external_context_bundle_round_trips(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    const BUNDLE_ID: &str = "scenario-7-bundle";

    let nats_url =
        std::env::var("CHOREO_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("choreo.deliberation.completed".to_owned())
        .await
        .context("subscribe choreo.deliberation.completed for scenario 7")?;
    nats.flush()
        .await
        .context("flush NATS subscribe (scenario 7)")?;

    let task_id = "e2e-task-7";
    let external_context = ExternalContextBundle {
        bundle_id: BUNDLE_ID.to_owned(),
        schema_version: "v1".to_owned(),
        summary: None,
        items: vec![],
        references: vec![],
        metadata: None,
    };

    let response = client
        .deliberate(DeliberateRequest {
            task: Some(Task {
                task_id: task_id.to_owned(),
                description: "scenario 7 ExternalContextBundle round-trip".to_owned(),
                specialty: specialty.to_owned(),
                constraints: None,
                attributes: None,
                external_context: Some(external_context),
                metadata: None,
            }),
        })
        .await
        .context("Deliberate (scenario 7) failed")?
        .into_inner();
    if response.winner_proposal_id.is_empty() {
        bail!("Deliberate did not return a winner for scenario 7");
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = remaining.min(Duration::from_secs(2));
        match tokio::time::timeout(chunk, subscription.next()).await {
            Ok(Some(msg)) => {
                let payload: serde_json::Value = serde_json::from_slice(&msg.payload)
                    .context("DeliberationCompleted (scenario 7) payload not JSON")?;
                let got_task = payload.get("task_id").and_then(|v| v.as_str());
                let got_bundle = payload
                    .get("external_context_bundle_id")
                    .and_then(|v| v.as_str());
                if got_task == Some(task_id) {
                    if got_bundle == Some(BUNDLE_ID) {
                        info!(
                            out_event_id = payload
                                .get("event_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or(""),
                            bundle_id = BUNDLE_ID,
                            "DeliberationCompleted carried the external_context_bundle_id"
                        );
                        return Ok(());
                    }
                    bail!(
                        "DeliberationCompleted for {task_id} missing external_context_bundle_id (got {got_bundle:?})"
                    );
                }
                warn!(
                    ?got_task,
                    "DeliberationCompleted seen for a different task_id; continuing"
                );
            }
            Ok(None) => {
                bail!("NATS subscription closed before scenario 7 envelope arrived");
            }
            Err(_) => {}
        }
    }
    bail!("no DeliberationCompleted for {task_id} arrived within 15s")
}
