//! Shared OTLP exporter initialization for MADE process hosts.

use anyhow::{Context as _, Result};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{WithExportConfig as _, WithTonicConfig as _};
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::SdkTracerProvider, Resource};
use opentelemetry_semantic_conventions as sc;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Owns the tracer provider so process shutdown flushes buffered spans.
#[must_use]
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    fn noop() -> Self {
        Self { provider: None }
    }
}

impl std::fmt::Debug for TelemetryGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TelemetryGuard").finish()
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

/// Install JSON tracing and, when `MADE_OTLP_ENDPOINT` is present, the shared
/// OTLP/gRPC exporter with the MADE mTLS environment contract.
pub fn init_otlp_tracing<W>(
    service_name: &'static str,
    service_version: &'static str,
    default_filter: &'static str,
    writer: W,
) -> Result<TelemetryGuard>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    global::set_text_map_propagator(TraceContextPropagator::new());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let fmt_layer = fmt::layer().json().with_writer(writer);

    let Some(endpoint) = read_env("MADE_OTLP_ENDPOINT") else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
        return Ok(TelemetryGuard::noop());
    };

    let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.clone());
    let mtls = if let Some(tls) = otlp_client_tls_from_env()? {
        exporter_builder = exporter_builder.with_tls_config(tls);
        true
    } else {
        false
    };
    let exporter = exporter_builder.build()?;
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_attributes([
                    opentelemetry::KeyValue::new(sc::resource::SERVICE_NAME, service_name),
                    opentelemetry::KeyValue::new(sc::resource::SERVICE_VERSION, service_version),
                ])
                .build(),
        )
        .build();
    global::set_tracer_provider(provider.clone());
    let otel_layer = tracing_opentelemetry::layer()
        .with_tracer(provider.tracer(service_name))
        .with_context_activation(false);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();
    tracing::info!(endpoint, mtls, "otlp exporter wired");
    Ok(TelemetryGuard {
        provider: Some(provider),
    })
}

fn otlp_client_tls_from_env(
) -> Result<Option<opentelemetry_otlp::tonic_types::transport::ClientTlsConfig>> {
    use opentelemetry_otlp::tonic_types::transport::{Certificate, ClientTlsConfig, Identity};

    let (Some(ca), Some(cert), Some(key)) = (
        read_env("MADE_OTLP_TLS_CA_PATH"),
        read_env("MADE_OTLP_TLS_CERT_PATH"),
        read_env("MADE_OTLP_TLS_KEY_PATH"),
    ) else {
        return Ok(None);
    };
    let ca_pem = std::fs::read(&ca).with_context(|| format!("reading OTLP CA {ca}"))?;
    let cert_pem = std::fs::read(&cert).with_context(|| format!("reading OTLP cert {cert}"))?;
    let key_pem = std::fs::read(&key).with_context(|| format!("reading OTLP key {key}"))?;
    let mut tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .identity(Identity::from_pem(cert_pem, key_pem));
    if let Some(domain) = read_env("MADE_OTLP_TLS_DOMAIN_NAME") {
        tls = tls.domain_name(domain);
    }
    Ok(Some(tls))
}

fn read_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
