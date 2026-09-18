//! Metrics adapters.
//!
//! A Prometheus-backed implementation of `MetricsRecorderPort`. Always
//! available (the `/metrics` endpoint is not feature-gated), holding an
//! explicit registry rather than a global recorder.

mod ceremony_concurrency_metrics;
mod prometheus_recorder;
mod prometheus_snapshot;

pub use prometheus_recorder::PrometheusMetricsRecorder;
