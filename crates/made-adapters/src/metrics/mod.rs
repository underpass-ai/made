//! Metrics adapters.
//!
//! A Prometheus-backed implementation of `MetricsRecorderPort`. Always
//! available (the `/metrics` endpoint is not feature-gated), holding an
//! explicit registry rather than a global recorder.

mod capacity_observations;
mod metric_registration;
mod prometheus_recorder;
mod prometheus_snapshot;
mod protocol_concurrency_metrics;

pub use capacity_observations::{CapacityObservations, CapacitySnapshot};
pub use prometheus_recorder::PrometheusMetricsRecorder;
