//! [`MetricsRecorderPort`] — in-process operational metrics sink.
//!
//! This port is deliberately distinct from [`StatisticsPort`]. That one
//! records a handful of durable business counters that may be backed by
//! Postgres, so it is `async` and fallible. This one is the sink for
//! rich, high-frequency operational metrics (latency histograms,
//! per-outcome counters) exported to Prometheus straight from process
//! memory.
//!
//! Recording an observation is a synchronous, lock-free, infallible
//! operation: it must never block a deliberation and must never fail it.
//! So the methods take `&self`, return nothing, and are not `async` —
//! instrumentation can never change a use case's control flow.
//!
//! The application layer depends only on this trait; the concrete metric
//! registry lives in an adapter. Use cases call `observe_*` / `record_*`
//! at the point where each measurement first becomes available.
//!
//! [`StatisticsPort`]: super::StatisticsPort

use crate::value_objects::{DeliberationOutcome, DurationMs, Score, Specialty};

pub trait MetricsRecorderPort: Send + Sync {
    /// Observe the end-to-end wall-clock duration of a deliberation that
    /// ran to completion, regardless of its terminal outcome.
    fn observe_deliberation_duration(&self, specialty: &Specialty, duration: DurationMs);

    /// Record the terminal [`DeliberationOutcome`] of a deliberation —
    /// the failure rate of the product surfaces here.
    fn record_deliberation_outcome(&self, specialty: &Specialty, outcome: DeliberationOutcome);

    /// Observe the score of the winning proposal. Recorded only on the
    /// success path (a `NoValidProposal` outcome has no winner).
    fn observe_winner_score(&self, specialty: &Specialty, score: Score);

    /// Observe the latency of a single LLM-judge rating call, whether it
    /// succeeded or failed. A call that times out reports a latency near
    /// the judge's deadline — the leading signal that the judge is
    /// approaching its timeout cliff.
    fn observe_judge_latency(&self, model: &str, duration: DurationMs);

    /// Observe a judge's `[0.0, 1.0]` verdict for one proposal — the
    /// basis for score calibration and threshold tuning.
    fn observe_judge_score(&self, model: &str, score: Score);
}

/// A [`MetricsRecorderPort`] that discards every observation.
///
/// The default sink wherever metrics are not the subject under test:
/// unit tests, benches, and any composition that does not export
/// Prometheus. Mirrors [`NullObserver`](super::NullObserver) for the
/// observer port.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMetricsRecorder;

impl MetricsRecorderPort for NoopMetricsRecorder {
    fn observe_deliberation_duration(&self, _specialty: &Specialty, _duration: DurationMs) {}
    fn record_deliberation_outcome(&self, _specialty: &Specialty, _outcome: DeliberationOutcome) {}
    fn observe_winner_score(&self, _specialty: &Specialty, _score: Score) {}
    fn observe_judge_latency(&self, _model: &str, _duration: DurationMs) {}
    fn observe_judge_score(&self, _model: &str, _score: Score) {}
}
