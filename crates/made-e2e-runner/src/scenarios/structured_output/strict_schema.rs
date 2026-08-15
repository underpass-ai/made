use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{Constraints, OrchestrateRequest, OutputContract, OutputFormat, Task};
use prost_types::value::Kind as PbKind;
use tonic::transport::Channel;
use tracing::{info, warn};

use super::super::pb_struct_from_pairs;

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
pub(crate) async fn verify_orchestrate_rejects_proposal_violating_json_schema(
    client: &mut MadeServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    // Pre-subscribe so the published TaskFailed envelope cannot land
    // before we are watching for it.
    let nats_url = std::env::var("MADE_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("made.task.failed".to_owned())
        .await
        .context("subscribe made.task.failed")?;
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
