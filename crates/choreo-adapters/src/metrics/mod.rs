//! Metrics adapters.
//!
//! A Prometheus-backed implementation of `MetricsRecorderPort`. Always
//! available (the `/metrics` endpoint is not feature-gated), holding an
//! explicit registry rather than a global recorder.

mod prometheus_recorder;

pub use prometheus_recorder::PrometheusMetricsRecorder;
