//! Operator tool: run a ceremony YAML against a deployed Choreographer.
//!
//! Reads a ceremony definition from a file and drives it through the
//! `RunCeremony` RPC, printing the terminal state, the per-step role and
//! status, and the rendered Mermaid conversation diagram. Useful for
//! exercising a ceremony (including provider-backed ones) against a live
//! deployment.
//!
//! Env:
//! - `CEREMONY_YAML_PATH` (required) — path to the ceremony YAML.
//! - `CHOREOGRAPHER_ENDPOINT` (default `http://localhost:50055`).
//! - `CEREMONY_ID` (default `operator-ceremony`).
//! - `CEREMONY_LEASE_TTL_MS` (default `120000`).

use anyhow::{Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::RunCeremonyRequest;

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = std::env::var("CHOREOGRAPHER_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:50055".to_owned());
    let path = std::env::var("CEREMONY_YAML_PATH").context("CEREMONY_YAML_PATH is required")?;
    let ceremony_id =
        std::env::var("CEREMONY_ID").unwrap_or_else(|_| "operator-ceremony".to_owned());
    let lease_ttl_ms = std::env::var("CEREMONY_LEASE_TTL_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(120_000);

    let definition_yaml =
        std::fs::read_to_string(&path).with_context(|| format!("reading ceremony YAML {path}"))?;

    let mut client = ChoreographerServiceClient::connect(endpoint.clone())
        .await
        .with_context(|| format!("connecting to {endpoint}"))?;

    let response = client
        .run_ceremony(RunCeremonyRequest {
            ceremony_id: ceremony_id.clone(),
            definition_yaml,
            context: None,
            lease_owner_id: "operator".to_owned(),
            lease_ttl_ms,
        })
        .await
        .context("RunCeremony RPC failed")?
        .into_inner();

    println!(
        "ceremony={} name={} final_state={} completed={} steps={}",
        response.ceremony_id,
        response.definition_name,
        response.final_state,
        response.completed,
        response.steps.len()
    );
    for step in &response.steps {
        println!(
            "  step={:<22} role={:<20} status={:<10} attempt={}",
            step.step_id, step.role_id, step.status, step.attempt
        );
    }
    println!(
        "--- conversation diagram ---\n{}",
        response.mermaid_sequence
    );

    if !response.completed {
        anyhow::bail!(
            "ceremony did not complete; final_state={}",
            response.final_state
        );
    }
    Ok(())
}
