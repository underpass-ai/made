//! [`Harness`] — the connected dependencies a chain needs:
//! a `tonic::Channel` toward MADE, and (optionally) an
//! `async_nats::Client` toward MADE's own bus.
//!
//! `nats_url` is intentionally `Option`. When omitted the chains
//! still run and the NATS-coupled assertions are recorded as
//! [`AssertionStatus::Skipped`] with an explicit reason — never
//! silently elided.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use made_proto::v1::made_service_client::MadeServiceClient;
use tonic::transport::{Channel, Endpoint};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct HarnessConfig {
    pub endpoint: String,
    pub nats_url: Option<String>,
    pub specialty: String,
    pub contract_id: String,
    pub connect_budget: Duration,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50055".to_owned(),
            nats_url: None,
            specialty: "triage".to_owned(),
            contract_id: "consumer-smoke-report-v1".to_owned(),
            connect_budget: Duration::from_secs(30),
        }
    }
}

pub struct Harness {
    pub grpc: MadeServiceClient<Channel>,
    pub nats: Option<async_nats::Client>,
}

impl Harness {
    /// Connect to the gRPC endpoint (retrying until the budget is
    /// exhausted) and, when configured, to NATS. NATS connect
    /// failures are NOT fatal — they downgrade the eventual
    /// NATS-coupled assertions to `Skipped` so a partial deploy
    /// still reports useful signal.
    pub async fn connect(cfg: &HarnessConfig) -> Result<Self> {
        let grpc = connect_with_retry(&cfg.endpoint, cfg.connect_budget).await?;
        let nats = match cfg.nats_url.as_deref() {
            None => None,
            Some(url) => match async_nats::connect(url).await {
                Ok(client) => {
                    info!(nats_url = url, "consumer-smoke connected to NATS");
                    Some(client)
                }
                Err(err) => {
                    warn!(
                        nats_url = url,
                        error = %err,
                        "consumer-smoke could not connect to NATS; bus assertions will be Skipped"
                    );
                    None
                }
            },
        };
        Ok(Self { grpc, nats })
    }

    /// Build a [`Harness`] from already-constructed parts. Used by the
    /// in-process integration tests that share a `tonic::Channel` from
    /// `GrpcFixture` (the retry loop in [`Harness::connect`] only
    /// makes sense against a real listener).
    #[must_use]
    pub fn from_parts(channel: Channel, nats: Option<async_nats::Client>) -> Self {
        Self {
            grpc: MadeServiceClient::new(channel),
            nats,
        }
    }
}

async fn connect_with_retry(
    endpoint: &str,
    budget: Duration,
) -> Result<MadeServiceClient<Channel>> {
    let deadline = std::time::Instant::now() + budget;
    let parsed: Endpoint = endpoint
        .parse()
        .with_context(|| format!("invalid gRPC endpoint: {endpoint}"))?;
    let mut last_err: Option<tonic::transport::Error> = None;
    while std::time::Instant::now() < deadline {
        match MadeServiceClient::connect(parsed.clone()).await {
            Ok(client) => {
                info!(endpoint, "consumer-smoke connected to made gRPC");
                return Ok(client);
            }
            Err(err) => {
                warn!(endpoint, error = %err, "gRPC not ready; will retry");
                last_err = Some(err);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(anyhow!(
        "could not connect to {endpoint} within {budget:?}: {last_err:?}"
    ))
}
