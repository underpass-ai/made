use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{DeliberateRequest, ExternalContextBundle, Task};
use tonic::transport::Channel;
use tracing::{info, warn};

/// Drives `Deliberate` with an `ExternalContextBundle` attached and
/// asserts that the outbound `made.deliberation.completed` envelope
/// carries the bundle id back. Closes the round-trip gap that
/// scenario 4 leaves open (scenario 4 covers causal ids; this one
/// covers the bundle pointer the kernel-shaped consumer would feed
/// in).
pub(crate) async fn verify_external_context_bundle_round_trips(
    client: &mut MadeServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    const BUNDLE_ID: &str = "scenario-7-bundle";

    let nats_url = std::env::var("MADE_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let nats = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    let mut subscription = nats
        .subscribe("made.deliberation.completed".to_owned())
        .await
        .context("subscribe made.deliberation.completed for scenario 7")?;
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
