//! Telemetry pipeline setup for the deployable MADE host.

use anyhow::Result;

#[cfg(feature = "otel")]
pub use made_adapters::telemetry::TelemetryGuard;

#[cfg(not(feature = "otel"))]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg(not(feature = "otel"))]
#[derive(Debug)]
#[must_use]
pub struct TelemetryGuard;

#[cfg(not(feature = "otel"))]
pub fn init_tracing() -> Result<TelemetryGuard> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
    Ok(TelemetryGuard)
}

#[cfg(feature = "otel")]
pub fn init_tracing() -> Result<TelemetryGuard> {
    made_adapters::telemetry::init_otlp_tracing(
        "made",
        env!("CARGO_PKG_VERSION"),
        "info",
        std::io::stdout,
    )
}
