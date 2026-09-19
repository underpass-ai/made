mod authorization_setup;
mod ceremony_children;
mod ceremony_diagram;
mod ceremony_progress;
mod ceremony_vllm;
mod ceremony_vllm_definition;
mod ceremony_vllm_provider_config;
mod children_ceremony_definitions;
mod connectivity;
mod daily_standup;
pub(crate) mod e2e_request_id_interceptor;
mod nats_subscription_ready;
mod pattern_ceremony_definition;
mod pattern_composition;
mod runtime;
mod speaker_talk_qa;
mod sprint_planning;
mod structured_output;
mod technical_debate;

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use made_proto::v1::made_service_client::MadeServiceClient;
use prost_types::{value::Kind as PbKind, Struct as PbStruct, Value as PbValue};
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tracing::{info, warn};

use e2e_request_id_interceptor::E2eRequestIdInterceptor;

pub(crate) use authorization_setup::provision_compose_business_grant;
pub(crate) use ceremony_children::verify_durable_children_over_public_rpc;
pub(crate) use ceremony_diagram::verify_editorial_meeting_ceremony_diagram;
pub(crate) use ceremony_progress::verify_live_ceremony_progress;
pub(crate) use ceremony_vllm::verify_editorial_meeting_ceremony_against_vllm_kind;
pub(crate) use connectivity::{
    verify_causal_metadata_propagates_over_nats, verify_delete_missing_council_returns_false,
    verify_deliberate_returns_winner, verify_seeded_council_visible,
};
pub(crate) use daily_standup::verify_daily_standup_ceremony;
pub(crate) use pattern_composition::{
    verify_concurrent_review_pattern, verify_incident_review_pattern,
};
pub(crate) use runtime::verify_orchestrate_invokes_runtime_executor;
pub(crate) use speaker_talk_qa::verify_speaker_talk_qa_ceremony;
pub(crate) use sprint_planning::verify_sprint_planning_ceremony;
pub(crate) use structured_output::{
    verify_external_context_bundle_round_trips, verify_multi_agent_council_against_real_vllm,
    verify_orchestrate_rejects_proposal_violating_json_schema,
    verify_structured_output_against_stub_llm, verify_structured_output_against_vllm_kind,
};
pub(crate) use technical_debate::verify_technical_debate_ceremony;

pub(crate) type E2eClient = MadeServiceClient<InterceptedService<Channel, E2eRequestIdInterceptor>>;

pub(crate) async fn connect_with_retry(
    endpoint: &str,
    total_budget: Duration,
) -> Result<E2eClient> {
    let deadline = std::time::Instant::now() + total_budget;
    let endpoint_parsed: Endpoint = endpoint
        .parse()
        .with_context(|| format!("invalid gRPC endpoint: {endpoint}"))?;
    let endpoint_parsed = configure_tls(endpoint_parsed, endpoint)?;

    let mut last_err: Option<tonic::transport::Error> = None;
    while std::time::Instant::now() < deadline {
        match endpoint_parsed.clone().connect().await {
            Ok(channel) => {
                info!(endpoint, "connected");
                return Ok(MadeServiceClient::with_interceptor(
                    channel,
                    E2eRequestIdInterceptor,
                ));
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

fn configure_tls(endpoint: Endpoint, endpoint_text: &str) -> Result<Endpoint> {
    let ca_path = std::env::var("MADE_CLIENT_TLS_CA_PATH").ok();
    let cert_path = std::env::var("MADE_CLIENT_TLS_CERT_PATH").ok();
    let key_path = std::env::var("MADE_CLIENT_TLS_KEY_PATH").ok();
    let configured = [ca_path.as_ref(), cert_path.as_ref(), key_path.as_ref()]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if configured == 0 {
        if endpoint_text.starts_with("https://") {
            return Err(anyhow!(
                "https MADE_ENDPOINT requires client CA, certificate, and key paths"
            ));
        }
        return Ok(endpoint);
    }
    if configured != 3 {
        return Err(anyhow!(
            "MADE_CLIENT_TLS_CA_PATH, MADE_CLIENT_TLS_CERT_PATH, and MADE_CLIENT_TLS_KEY_PATH must be set together"
        ));
    }
    let ca = std::fs::read(ca_path.unwrap()).context("read MADE client CA")?;
    let certificate = std::fs::read(cert_path.unwrap()).context("read MADE client certificate")?;
    let key = std::fs::read(key_path.unwrap()).context("read MADE client key")?;
    let domain = std::env::var("MADE_CLIENT_TLS_DOMAIN").unwrap_or_else(|_| "made".to_owned());
    endpoint
        .tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(ca))
                .identity(Identity::from_pem(certificate, key))
                .domain_name(domain),
        )
        .context("configure MADE client mTLS")
}

fn pb_struct_from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, PbKind)>) -> PbStruct {
    PbStruct {
        fields: pairs
            .into_iter()
            .map(|(k, kind)| (k.to_owned(), PbValue { kind: Some(kind) }))
            .collect(),
    }
}
