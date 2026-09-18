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
    CeremonyOutcome, DeliberationOutcome, Discrimination, DurationMs, LlmErrorKind, RecorderName,
    Score, ScoringMode, Specialty, StepStatus, TokenUsage,
};

pub trait MetricsRecorderPort: Send + Sync {
    /// What is recording, in one lower-case word — `noop`, `prometheus`.
    ///
    /// A status answer that did not say this would look the same whether
    /// the host wired a registry or nothing at all, which is the one
    /// question an operator asks of an edition whose recorder is the
    /// host's choice. Required rather than defaulted: a recorder that
    /// forgot to name itself would answer `unknown`, and there is no
    /// honest value for that.
    fn recorder_name(&self) -> RecorderName;

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

    /// Record that a worker claimed a ceremony step from the stream.
    fn record_ceremony_step_claimed(&self, _ceremony: &str, _step: &str) {}

    /// Peak live claims within a completed state visit/iteration, from sealed facts.
    fn observe_ceremony_claim_peak_width(
        &self,
        _ceremony: &str,
        _state: &str,
        _width: crate::value_objects::MaxParallel,
    ) {
    }

    /// A typed failure of a sibling; never inferred from an error message.
    fn record_ceremony_sibling_failure(
        &self,
        _ceremony: &str,
        _step: &str,
        _kind: crate::value_objects::StepFailureKind,
    ) {
    }

    /// Record the attempt number assigned to a step claim.
    fn record_ceremony_step_attempt(&self, _ceremony: &str, _step: &str, _attempt: u32) {}

    /// Record the semantic iteration assigned to a step claim.
    fn record_ceremony_step_iteration(&self, _ceremony: &str, _step: &str, _iteration: u32) {}

    /// Record the outcome of a human guard decision.
    fn record_ceremony_guard_decided(&self, _ceremony: &str, _guard: &str, _decision: &str) {}

    /// Record that a dynamic intervention was opened.
    fn record_ceremony_intervention_opened(&self, _ceremony: &str, _kind: &str) {}

    /// Record that a dynamic intervention received an answer.
    fn record_ceremony_intervention_answered(&self, _ceremony: &str) {}

    /// Record an applied ceremony state transition.
    fn record_ceremony_transition_applied(
        &self,
        _ceremony: &str,
        _from_state: &str,
        _to_state: &str,
    ) {
    }

    /// Record that a step lease was acquired.
    fn record_ceremony_lease_acquired(&self, _ceremony: &str, _step: &str) {}

    /// Record that a prior step lease expired before a replacement claim.
    fn record_ceremony_lease_expired(&self, _ceremony: &str, _step: &str) {}

    /// Observe the latency of one NATS publish, by event `subject_kind`.
    fn observe_nats_publish(&self, subject_kind: &str, duration: DurationMs);

    /// Record a failed NATS publish, by `subject_kind` and `reason`
    /// (serialize vs publish). Completion events drive downstream
    /// orchestration, so a silent publish failure loses work.
    fn record_nats_publish_error(&self, subject_kind: &str, reason: &str);

    /// Set the gauge of Postgres connections currently checked out of the
    /// pool. Sampled at scrape time; saturation here cascades into
    /// readiness failures, so it is the leading pool-exhaustion signal.
    fn set_postgres_pool_in_use(&self, connections: i64);

    /// Record which branch a judge-aware scorer took for one proposal. A
    /// spike in `uniform_fallback` while a judge is configured means the
    /// judge silently is not scoring — an otherwise-undetectable misconfig.
    fn record_scoring_mode(&self, mode: ScoringMode);
}
