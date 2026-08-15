use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{DeleteCouncilRequest, DeliberateRequest, ListCouncilsRequest, Task};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tonic::transport::Channel;
use tracing::{info, warn};

pub(crate) async fn verify_seeded_council_visible(
    client: &mut MadeServiceClient<Channel>,
    seed_specialty: &str,
) -> Result<()> {
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
            "expected at least one seeded council — did the MADE start with MADE_SEED_SPECIALTIES?"
        );
    }
    if !councils.iter().any(|c| c.specialty == seed_specialty) {
        bail!(
            "seeded specialty {seed_specialty} not found among {:?}",
            councils.iter().map(|c| &c.specialty).collect::<Vec<_>>()
        );
    }
    Ok(())
}

pub(crate) async fn verify_deliberate_returns_winner(
    client: &mut MadeServiceClient<Channel>,
    seed_specialty: &str,
) -> Result<()> {
    let response = client
        .deliberate(DeliberateRequest {
            task: Some(Task {
                task_id: "e2e-task-1".to_owned(),
                specialty: seed_specialty.to_owned(),
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
    Ok(())
}

pub(crate) async fn verify_delete_missing_council_returns_false(
    client: &mut MadeServiceClient<Channel>,
) -> Result<()> {
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
    Ok(())
}

/// Publishes a `TriggerEvent` on NATS with known `correlation_id` and
/// `causation_id`, then asserts that the resulting
/// `DeliberationCompleted` envelope on the outbound bus carries the
/// same causal ids. Closes the stack-E2E loose end of Epic 5.
pub(crate) async fn verify_causal_metadata_propagates_over_nats(specialty: &str) -> Result<()> {
    let nats_url = std::env::var("MADE_NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_owned());
    let client = async_nats::connect(&nats_url)
        .await
        .with_context(|| format!("connect NATS at {nats_url}"))?;
    info!(nats_url, "connected to NATS for causal metadata assertion");

    let mut subscription = client
        .subscribe("made.deliberation.completed".to_owned())
        .await
        .context("subscribe made.deliberation.completed")?;
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
    let subject = format!("made.trigger.{specialty}");
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
