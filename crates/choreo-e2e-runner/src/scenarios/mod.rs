mod ceremony_diagram;
mod ceremony_vllm;
mod ceremony_vllm_definition;
mod ceremony_vllm_provider_config;
mod connectivity;
mod daily_standup;
mod runtime;
mod structured_output;

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use prost_types::{value::Kind as PbKind, Struct as PbStruct, Value as PbValue};
use tonic::transport::{Channel, Endpoint};
use tracing::{info, warn};

pub(crate) use ceremony_diagram::verify_editorial_meeting_ceremony_diagram;
pub(crate) use ceremony_vllm::verify_editorial_meeting_ceremony_against_vllm_kind;
pub(crate) use connectivity::{
    verify_causal_metadata_propagates_over_nats, verify_delete_missing_council_returns_false,
    verify_deliberate_returns_winner, verify_seeded_council_visible,
};
pub(crate) use daily_standup::verify_daily_standup_ceremony;
pub(crate) use runtime::verify_orchestrate_invokes_runtime_executor;
pub(crate) use structured_output::{
    verify_external_context_bundle_round_trips, verify_multi_agent_council_against_real_vllm,
    verify_orchestrate_rejects_proposal_violating_json_schema,
    verify_structured_output_against_stub_llm, verify_structured_output_against_vllm_kind,
};

pub(crate) async fn connect_with_retry(
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

fn pb_struct_from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, PbKind)>) -> PbStruct {
    PbStruct {
        fields: pairs
            .into_iter()
            .map(|(k, kind)| (k.to_owned(), PbValue { kind: Some(kind) }))
            .collect(),
    }
}
