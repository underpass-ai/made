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

use crate::value_objects::{
    CeremonyOutcome, DeliberationOutcome, Discrimination, DurationMs, LlmErrorKind, Score,
    Specialty, StepStatus, TokenUsage,
};

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

    /// Record a failed LLM-judge call, classified by [`LlmErrorKind`].
    /// A judge error fails the deliberation at the validating gate, and
    /// the kind dictates the response (timeout vs rate-limit vs
    /// credential rotation).
    fn record_judge_error(&self, model: &str, kind: LlmErrorKind);

    /// Record a failed proposing-agent call, partitioned by `provider`
    /// (vllm / openai / anthropic) and [`LlmErrorKind`]. A 429 here is
    /// backpressure into every deliberation that routes to the provider.
    fn record_provider_error(&self, provider: &str, kind: LlmErrorKind);

    /// Record token usage for one LLM-judge call. Combined with judge
    /// discrimination, this is the cost side of the judge's ROI.
    fn record_judge_tokens(&self, model: &str, usage: TokenUsage);

    /// Record token usage for one proposing-agent call, by provider —
    /// cost attribution and prompt-bloat detection.
    fn record_provider_tokens(&self, provider: &str, usage: TokenUsage);

    /// Observe the latency of one proposing-agent call, by `provider` and
    /// `operation` (generate / critique / revise). Recorded on every path.
    fn observe_provider_request(&self, provider: &str, operation: &str, duration: DurationMs);

    /// Increment the in-flight gauge for `provider` at the start of a
    /// call. With vLLM serialised (`max-num-seqs=1`), a sustained
    /// in-flight depth above 1 is the leading indicator of the latency
    /// cliff — concurrency that any other backend would absorb queues here.
    fn inc_provider_in_flight(&self, provider: &str);

    /// Decrement the in-flight gauge for `provider` when a call returns,
    /// whether it succeeded or failed.
    fn dec_provider_in_flight(&self, provider: &str);

    /// Record whether the scoring policy re-ranked the winner of a
    /// deliberation. The killer signal for the LLM judge's worth: a
    /// `reranked` ratio near zero means the judge never changes the
    /// outcome and is pure cost.
    fn record_discrimination(&self, specialty: &Specialty, result: Discrimination);

    /// Record the terminal outcome of a ceremony run, by definition name —
    /// the completion-rate and failure-mode split per ceremony type.
    fn record_ceremony_outcome(&self, ceremony: &str, outcome: CeremonyOutcome);

    /// Observe the end-to-end duration of a ceremony run that reached a
    /// terminal state.
    fn observe_ceremony_duration(&self, ceremony: &str, duration: DurationMs);

    /// Observe the duration of a single ceremony step — slow-step
    /// isolation within a ceremony.
    fn observe_ceremony_step_duration(&self, ceremony: &str, step: &str, duration: DurationMs);

    /// Record that a ceremony step finished with `status` — per-step volume
    /// and the failure split.
    fn record_ceremony_step(&self, ceremony: &str, step: &str, status: StepStatus);

    /// Record that no transition out of `from_state` was satisfiable — a
    /// guard deadlock or a missing event, distinct from a normal pending
    /// wait.
    fn record_ceremony_transition_blocked(&self, ceremony: &str, from_state: &str);
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
    fn record_judge_error(&self, _model: &str, _kind: LlmErrorKind) {}
    fn record_provider_error(&self, _provider: &str, _kind: LlmErrorKind) {}
    fn record_judge_tokens(&self, _model: &str, _usage: TokenUsage) {}
    fn record_provider_tokens(&self, _provider: &str, _usage: TokenUsage) {}
    fn observe_provider_request(&self, _provider: &str, _operation: &str, _duration: DurationMs) {}
    fn inc_provider_in_flight(&self, _provider: &str) {}
    fn dec_provider_in_flight(&self, _provider: &str) {}
    fn record_discrimination(&self, _specialty: &Specialty, _result: Discrimination) {}
    fn record_ceremony_outcome(&self, _ceremony: &str, _outcome: CeremonyOutcome) {}
    fn observe_ceremony_duration(&self, _ceremony: &str, _duration: DurationMs) {}
    fn observe_ceremony_step_duration(&self, _ceremony: &str, _step: &str, _duration: DurationMs) {}
    fn record_ceremony_step(&self, _ceremony: &str, _step: &str, _status: StepStatus) {}
    fn record_ceremony_transition_blocked(&self, _ceremony: &str, _from_state: &str) {}
}
