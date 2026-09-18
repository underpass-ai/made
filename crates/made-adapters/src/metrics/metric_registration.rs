use made_core::error::DomainError;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry};

/// Define a labelled histogram, register it on `registry`, and return
/// the handle. Definition and registration are paired so a metric can
/// never be defined but left unregistered (it would then be invisible at
/// `/metrics`).
pub(super) fn register_histogram(
    registry: &Registry,
    name: &str,
    help: &str,
    buckets: &[f64],
    labels: &[&str],
) -> Result<HistogramVec, DomainError> {
    let metric = HistogramVec::new(
        HistogramOpts::new(name, help).buckets(buckets.to_vec()),
        labels,
    )
    .map_err(|err| metrics_error(&err))?;
    registry
        .register(Box::new(metric.clone()))
        .map_err(|err| metrics_error(&err))?;
    Ok(metric)
}

/// Define and register a labelled counter; see [`register_histogram`].
pub(super) fn register_counter(
    registry: &Registry,
    name: &str,
    help: &str,
    labels: &[&str],
) -> Result<IntCounterVec, DomainError> {
    let metric =
        IntCounterVec::new(Opts::new(name, help), labels).map_err(|err| metrics_error(&err))?;
    registry
        .register(Box::new(metric.clone()))
        .map_err(|err| metrics_error(&err))?;
    Ok(metric)
}

/// Define and register a labelled gauge; see [`register_histogram`].
pub(super) fn register_gauge(
    registry: &Registry,
    name: &str,
    help: &str,
    labels: &[&str],
) -> Result<IntGaugeVec, DomainError> {
    let metric =
        IntGaugeVec::new(Opts::new(name, help), labels).map_err(|err| metrics_error(&err))?;
    registry
        .register(Box::new(metric.clone()))
        .map_err(|err| metrics_error(&err))?;
    Ok(metric)
}

/// Map a Prometheus setup failure to a fail-fast wiring error.
pub(super) fn metrics_error(err: &prometheus::Error) -> DomainError {
    tracing::error!(error = %err, "prometheus metrics setup failed");
    DomainError::InvariantViolated {
        reason: "prometheus metrics setup failed",
    }
}
