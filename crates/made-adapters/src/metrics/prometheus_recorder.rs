//! [`PrometheusMetricsRecorder`] — Prometheus-backed [`MetricsRecorderPort`].
//!
//! Holds an explicit [`Registry`] and a typed handle per metric family —
//! no global recorder, so the composition root owns the metrics' lifetime
//! like every other adapter. Recording is lock-free (Prometheus uses
//! atomics under the hood), matching the port's infallible, synchronous
//! contract. The binary's `/metrics` handler renders the registry through
//! [`render`](PrometheusMetricsRecorder::render).
//!
//! Unit conventions: latency histograms are in **seconds**; scores are the
//! raw `[0.0, 1.0]` value. Conversion from the domain's millisecond
//! durations happens here so the port stays domain-typed.

use made_core::error::DomainError;
use made_core::ports::MetricsRecorderPort;
use made_core::value_objects::{
    CeremonyOutcome, DeliberationOutcome, Discrimination, DurationMs, LlmErrorKind, Score,
    ScoringMode, Specialty, StepStatus, TokenUsage,
};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
    TextEncoder,
};

/// Latency buckets (seconds) sized for serialized vLLM deliberations,
/// which run from a second or two up into minutes.
const DURATION_BUCKETS_SECONDS: &[f64] = &[
    0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 45.0, 60.0, 120.0, 300.0,
];

/// Score buckets across the closed `[0.0, 1.0]` range.
const SCORE_BUCKETS: &[f64] = &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

/// Latency buckets (seconds) for judge rating calls, weighted toward the
/// upper end so a p99 creeping toward the judge's 60s timeout is visible.
const JUDGE_LATENCY_BUCKETS_SECONDS: &[f64] =
    &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 45.0, 55.0, 60.0];

/// Latency buckets (seconds) for a single proposing-agent call. A
/// generate/critique/revise on a serialised vLLM model can run to tens of
/// seconds, so the range reaches past two minutes.
const PROVIDER_LATENCY_BUCKETS_SECONDS: &[f64] = &[
    0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0,
];

/// Latency buckets (seconds) for a NATS publish — a fast in-cluster
/// operation, so the range is sub-second-weighted.
const NATS_PUBLISH_BUCKETS_SECONDS: &[f64] =
    &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5];

pub struct PrometheusMetricsRecorder {
    registry: Registry,
    deliberation_duration_seconds: HistogramVec,
    deliberation_winner_score: HistogramVec,
    deliberation_completed_total: IntCounterVec,
    judge_latency_seconds: HistogramVec,
    judge_score: HistogramVec,
    judge_errors_total: IntCounterVec,
    provider_errors_total: IntCounterVec,
    judge_tokens_total: IntCounterVec,
    provider_tokens_total: IntCounterVec,
    provider_request_duration_seconds: HistogramVec,
    provider_in_flight: IntGaugeVec,
    judge_discrimination_total: IntCounterVec,
    ceremony_completed_total: IntCounterVec,
    ceremony_duration_seconds: HistogramVec,
    ceremony_step_duration_seconds: HistogramVec,
    ceremony_step_total: IntCounterVec,
    ceremony_transition_blocked_total: IntCounterVec,
    nats_publish_duration_seconds: HistogramVec,
    nats_publish_errors_total: IntCounterVec,
    postgres_pool_in_use: IntGauge,
    judge_scoring_mode_total: IntCounterVec,
}

impl PrometheusMetricsRecorder {
    /// Build the recorder, defining and registering every metric family.
    ///
    /// # Errors
    ///
    /// Returns a [`DomainError::InvariantViolated`] if a metric fails to
    /// register — a duplicate name or malformed label is a wiring bug,
    /// surfaced at composition time rather than silently at runtime.
    // A flat define-and-register list, one entry per metric family; it
    // grows by enumeration, not complexity.
    #[allow(clippy::too_many_lines)]
    pub fn new() -> Result<Self, DomainError> {
        let registry = Registry::new();
        let deliberation_duration_seconds = register_histogram(
            &registry,
            "made_deliberation_duration_seconds",
            "End-to-end wall-clock duration of a deliberation that ran to completion.",
            DURATION_BUCKETS_SECONDS,
            &["specialty"],
        )?;
        let deliberation_winner_score = register_histogram(
            &registry,
            "made_deliberation_winner_score",
            "Score of the winning proposal of a deliberation.",
            SCORE_BUCKETS,
            &["specialty"],
        )?;
        let deliberation_completed_total = register_counter(
            &registry,
            "made_deliberation_completed_total",
            "Deliberations that ran to completion, partitioned by terminal outcome.",
            &["specialty", "outcome"],
        )?;
        let judge_latency_seconds = register_histogram(
            &registry,
            "made_judge_latency_seconds",
            "Latency of a single LLM-judge rating call, success or failure.",
            JUDGE_LATENCY_BUCKETS_SECONDS,
            &["model"],
        )?;
        let judge_score = register_histogram(
            &registry,
            "made_judge_score",
            "Distribution of the LLM judge's 0.0-1.0 verdicts.",
            SCORE_BUCKETS,
            &["model"],
        )?;
        let judge_errors_total = register_counter(
            &registry,
            "made_judge_errors_total",
            "Failed LLM-judge calls, partitioned by error classification.",
            &["model", "error_kind"],
        )?;
        let provider_errors_total = register_counter(
            &registry,
            "made_provider_errors_total",
            "Failed proposing-agent calls, partitioned by provider and error classification.",
            &["provider", "error_kind"],
        )?;
        let judge_tokens_total = register_counter(
            &registry,
            "made_judge_tokens_total",
            "Tokens consumed by LLM-judge calls, partitioned by token type.",
            &["model", "token_type"],
        )?;
        let provider_tokens_total = register_counter(
            &registry,
            "made_provider_tokens_total",
            "Tokens consumed by proposing-agent calls, partitioned by provider and token type.",
            &["provider", "token_type"],
        )?;
        let provider_request_duration_seconds = register_histogram(
            &registry,
            "made_provider_request_duration_seconds",
            "Latency of a single proposing-agent call, by provider and operation.",
            PROVIDER_LATENCY_BUCKETS_SECONDS,
            &["provider", "operation"],
        )?;
        let provider_in_flight = register_gauge(
            &registry,
            "made_provider_in_flight",
            "In-flight proposing-agent calls per provider; the vLLM serial-saturation signal.",
            &["provider"],
        )?;
        let judge_discrimination_total = register_counter(
            &registry,
            "made_judge_discrimination_total",
            "Whether the scoring policy re-ranked the deliberation winner vs the structural baseline.",
            &["specialty", "result"],
        )?;
        let ceremony_completed_total = register_counter(
            &registry,
            "made_ceremony_completed_total",
            "Ceremony runs that reached a stop, partitioned by terminal outcome.",
            &["ceremony", "outcome"],
        )?;
        let ceremony_duration_seconds = register_histogram(
            &registry,
            "made_ceremony_duration_seconds",
            "End-to-end duration of a ceremony run that reached a terminal state.",
            DURATION_BUCKETS_SECONDS,
            &["ceremony"],
        )?;
        let ceremony_step_duration_seconds = register_histogram(
            &registry,
            "made_ceremony_step_duration_seconds",
            "Duration of a single ceremony step.",
            DURATION_BUCKETS_SECONDS,
            &["ceremony", "step"],
        )?;
        let ceremony_step_total = register_counter(
            &registry,
            "made_ceremony_step_total",
            "Ceremony steps that finished, partitioned by step and terminal status.",
            &["ceremony", "step", "status"],
        )?;
        let ceremony_transition_blocked_total = register_counter(
            &registry,
            "made_ceremony_transition_blocked_total",
            "Ceremony states from which no transition was satisfiable — a deadlock or missing event.",
            &["ceremony", "from_state"],
        )?;
        let nats_publish_duration_seconds = register_histogram(
            &registry,
            "made_nats_publish_duration_seconds",
            "Latency of a NATS publish, by event subject kind.",
            NATS_PUBLISH_BUCKETS_SECONDS,
            &["subject_kind"],
        )?;
        let nats_publish_errors_total = register_counter(
            &registry,
            "made_nats_publish_errors_total",
            "Failed NATS publishes, by subject kind and reason.",
            &["subject_kind", "reason"],
        )?;
        let postgres_pool_in_use = IntGauge::new(
            "made_postgres_pool_in_use",
            "Connections currently checked out of the Postgres pool.",
        )
        .map_err(|err| metrics_error(&err))?;
        registry
            .register(Box::new(postgres_pool_in_use.clone()))
            .map_err(|err| metrics_error(&err))?;
        let judge_scoring_mode_total = register_counter(
            &registry,
            "made_judge_scoring_mode_total",
            "Judge-aware scoring decisions, by mode (judge verdict vs uniform fallback).",
            &["mode"],
        )?;

        Ok(Self {
            registry,
            deliberation_duration_seconds,
            deliberation_winner_score,
            deliberation_completed_total,
            judge_latency_seconds,
            judge_score,
            judge_errors_total,
            provider_errors_total,
            judge_tokens_total,
            provider_tokens_total,
            provider_request_duration_seconds,
            provider_in_flight,
            judge_discrimination_total,
            ceremony_completed_total,
            ceremony_duration_seconds,
            ceremony_step_duration_seconds,
            ceremony_step_total,
            ceremony_transition_blocked_total,
            nats_publish_duration_seconds,
            nats_publish_errors_total,
            postgres_pool_in_use,
            judge_scoring_mode_total,
        })
    }

    /// Render every registered metric in the Prometheus text exposition
    /// format. Returns an empty string (and logs) on the practically
    /// impossible event that text encoding fails, so the `/metrics`
    /// handler can never be brought down by instrumentation.
    #[must_use]
    pub fn render(&self) -> String {
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        if let Err(err) = encoder.encode(&metric_families, &mut buffer) {
            tracing::error!(error = %err, "prometheus metrics encode failed");
            return String::new();
        }
        String::from_utf8(buffer).unwrap_or_default()
    }
}

impl std::fmt::Debug for PrometheusMetricsRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The metric handles are noisy and carry no useful debug signal;
        // the registry's contents are observable at `/metrics` instead.
        f.debug_struct("PrometheusMetricsRecorder").finish()
    }
}

impl MetricsRecorderPort for PrometheusMetricsRecorder {
    fn observe_deliberation_duration(&self, specialty: &Specialty, duration: DurationMs) {
        self.deliberation_duration_seconds
            .with_label_values(&[specialty.as_str()])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn record_deliberation_outcome(&self, specialty: &Specialty, outcome: DeliberationOutcome) {
        self.deliberation_completed_total
            .with_label_values(&[specialty.as_str(), outcome.as_label()])
            .inc();
    }

    fn observe_winner_score(&self, specialty: &Specialty, score: Score) {
        self.deliberation_winner_score
            .with_label_values(&[specialty.as_str()])
            .observe(score.get());
    }

    fn observe_judge_latency(&self, model: &str, duration: DurationMs) {
        self.judge_latency_seconds
            .with_label_values(&[model])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn observe_judge_score(&self, model: &str, score: Score) {
        self.judge_score
            .with_label_values(&[model])
            .observe(score.get());
    }

    fn record_judge_error(&self, model: &str, kind: LlmErrorKind) {
        self.judge_errors_total
            .with_label_values(&[model, kind.as_label()])
            .inc();
    }

    fn record_provider_error(&self, provider: &str, kind: LlmErrorKind) {
        self.provider_errors_total
            .with_label_values(&[provider, kind.as_label()])
            .inc();
    }

    fn record_judge_tokens(&self, model: &str, usage: TokenUsage) {
        self.judge_tokens_total
            .with_label_values(&[model, "prompt"])
            .inc_by(u64::from(usage.prompt()));
        self.judge_tokens_total
            .with_label_values(&[model, "completion"])
            .inc_by(u64::from(usage.completion()));
    }

    fn record_provider_tokens(&self, provider: &str, usage: TokenUsage) {
        self.provider_tokens_total
            .with_label_values(&[provider, "prompt"])
            .inc_by(u64::from(usage.prompt()));
        self.provider_tokens_total
            .with_label_values(&[provider, "completion"])
            .inc_by(u64::from(usage.completion()));
    }

    fn observe_provider_request(&self, provider: &str, operation: &str, duration: DurationMs) {
        self.provider_request_duration_seconds
            .with_label_values(&[provider, operation])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn inc_provider_in_flight(&self, provider: &str) {
        self.provider_in_flight.with_label_values(&[provider]).inc();
    }

    fn dec_provider_in_flight(&self, provider: &str) {
        self.provider_in_flight.with_label_values(&[provider]).dec();
    }

    fn record_discrimination(&self, specialty: &Specialty, result: Discrimination) {
        self.judge_discrimination_total
            .with_label_values(&[specialty.as_str(), result.as_label()])
            .inc();
    }

    fn record_ceremony_outcome(&self, ceremony: &str, outcome: CeremonyOutcome) {
        self.ceremony_completed_total
            .with_label_values(&[ceremony, outcome.as_label()])
            .inc();
    }

    fn observe_ceremony_duration(&self, ceremony: &str, duration: DurationMs) {
        self.ceremony_duration_seconds
            .with_label_values(&[ceremony])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn observe_ceremony_step_duration(&self, ceremony: &str, step: &str, duration: DurationMs) {
        self.ceremony_step_duration_seconds
            .with_label_values(&[ceremony, step])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn record_ceremony_step(&self, ceremony: &str, step: &str, status: StepStatus) {
        self.ceremony_step_total
            .with_label_values(&[ceremony, step, status.as_label()])
            .inc();
    }

    fn record_ceremony_transition_blocked(&self, ceremony: &str, from_state: &str) {
        self.ceremony_transition_blocked_total
            .with_label_values(&[ceremony, from_state])
            .inc();
    }

    fn observe_nats_publish(&self, subject_kind: &str, duration: DurationMs) {
        self.nats_publish_duration_seconds
            .with_label_values(&[subject_kind])
            .observe(duration.get() as f64 / 1000.0);
    }

    fn record_nats_publish_error(&self, subject_kind: &str, reason: &str) {
        self.nats_publish_errors_total
            .with_label_values(&[subject_kind, reason])
            .inc();
    }

    fn set_postgres_pool_in_use(&self, connections: i64) {
        self.postgres_pool_in_use.set(connections);
    }

    fn record_scoring_mode(&self, mode: ScoringMode) {
        self.judge_scoring_mode_total
            .with_label_values(&[mode.as_label()])
            .inc();
    }
}

/// Define a labelled histogram, register it on `registry`, and return
/// the handle. Definition and registration are paired so a metric can
/// never be defined but left unregistered (it would then be invisible at
/// `/metrics`).
fn register_histogram(
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

/// Define a labelled counter, register it on `registry`, and return the
/// handle. See [`register_histogram`] for the pairing rationale.
fn register_counter(
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

/// Define a labelled gauge, register it on `registry`, and return the
/// handle. See [`register_histogram`] for the pairing rationale.
fn register_gauge(
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
fn metrics_error(err: &prometheus::Error) -> DomainError {
    tracing::error!(error = %err, "prometheus metrics setup failed");
    DomainError::InvariantViolated {
        reason: "prometheus metrics setup failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specialty() -> Specialty {
        Specialty::new("reviewer").unwrap()
    }

    #[test]
    fn registered_families_render_their_type_metadata_once_observed() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        // Prometheus omits metric families with no observed label sets, so
        // touch each one before asserting it is exposed.
        recorder.observe_deliberation_duration(&specialty(), DurationMs::from_millis(1_000));
        recorder.observe_winner_score(&specialty(), Score::new(0.5).unwrap());
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::Success);
        recorder.observe_judge_latency("gemma", DurationMs::from_millis(1_000));
        recorder.observe_judge_score("gemma", Score::new(0.5).unwrap());
        recorder.record_judge_error("gemma", LlmErrorKind::Timeout);
        recorder.record_provider_error("vllm", LlmErrorKind::RateLimited);
        recorder.record_judge_tokens("gemma", TokenUsage::new(10, 5));
        recorder.record_provider_tokens("vllm", TokenUsage::new(10, 5));
        recorder.observe_provider_request("vllm", "generate", DurationMs::from_millis(1_000));
        recorder.inc_provider_in_flight("vllm");
        recorder.record_discrimination(&specialty(), Discrimination::Reranked);
        recorder.record_ceremony_outcome("plan", CeremonyOutcome::Completed);
        recorder.observe_ceremony_duration("plan", DurationMs::from_millis(1_000));
        recorder.observe_ceremony_step_duration("plan", "frame", DurationMs::from_millis(1_000));
        recorder.record_ceremony_step("plan", "frame", StepStatus::Completed);
        recorder.record_ceremony_transition_blocked("plan", "drafting");
        recorder.observe_nats_publish("deliberation_completed", DurationMs::from_millis(5));
        recorder.record_nats_publish_error("deliberation_completed", "publish");

        let text = recorder.render();
        assert!(text.contains("# TYPE made_deliberation_duration_seconds histogram"));
        assert!(text.contains("# TYPE made_deliberation_winner_score histogram"));
        assert!(text.contains("# TYPE made_deliberation_completed_total counter"));
        assert!(text.contains("# TYPE made_judge_latency_seconds histogram"));
        assert!(text.contains("# TYPE made_judge_score histogram"));
        assert!(text.contains("# TYPE made_judge_errors_total counter"));
        assert!(text.contains("# TYPE made_provider_errors_total counter"));
        assert!(text.contains("# TYPE made_judge_tokens_total counter"));
        assert!(text.contains("# TYPE made_provider_tokens_total counter"));
        assert!(text.contains("# TYPE made_provider_request_duration_seconds histogram"));
        assert!(text.contains("# TYPE made_provider_in_flight gauge"));
        assert!(text.contains("# TYPE made_judge_discrimination_total counter"));
        assert!(text.contains("# TYPE made_ceremony_completed_total counter"));
        assert!(text.contains("# TYPE made_ceremony_duration_seconds histogram"));
        assert!(text.contains("# TYPE made_ceremony_step_duration_seconds histogram"));
        assert!(text.contains("# TYPE made_ceremony_step_total counter"));
        assert!(text.contains("# TYPE made_ceremony_transition_blocked_total counter"));
        assert!(text.contains("# TYPE made_nats_publish_duration_seconds histogram"));
        assert!(text.contains("# TYPE made_nats_publish_errors_total counter"));
        // The single label-less pool gauge renders even unset.
        assert!(text.contains("# TYPE made_postgres_pool_in_use gauge"));
        recorder.record_scoring_mode(ScoringMode::JudgeVerdict);
        assert!(recorder
            .render()
            .contains("# TYPE made_judge_scoring_mode_total counter"));
    }

    #[test]
    fn records_scoring_mode_by_branch() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_scoring_mode(ScoringMode::JudgeVerdict);
        recorder.record_scoring_mode(ScoringMode::JudgeVerdict);
        recorder.record_scoring_mode(ScoringMode::UniformFallback);

        let text = recorder.render();
        assert!(text.contains("made_judge_scoring_mode_total{mode=\"judge_verdict\"} 2"));
        assert!(text.contains("made_judge_scoring_mode_total{mode=\"uniform_fallback\"} 1"));
    }

    #[test]
    fn postgres_pool_gauge_reflects_the_last_set_value() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.set_postgres_pool_in_use(7);
        assert!(recorder.render().contains("made_postgres_pool_in_use 7"));
        recorder.set_postgres_pool_in_use(3);
        assert!(recorder.render().contains("made_postgres_pool_in_use 3"));
    }

    #[test]
    fn records_nats_publish_errors_by_subject_and_reason() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_nats_publish_error("deliberation_completed", "publish");
        recorder.record_nats_publish_error("deliberation_completed", "publish");
        recorder.record_nats_publish_error("task_dispatched", "serialize");

        let text = recorder.render();
        assert!(text.contains(
            "made_nats_publish_errors_total{reason=\"publish\",subject_kind=\"deliberation_completed\"} 2"
        ));
        assert!(text.contains(
            "made_nats_publish_errors_total{reason=\"serialize\",subject_kind=\"task_dispatched\"} 1"
        ));
    }

    #[test]
    fn records_ceremony_outcome_and_step_status() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_ceremony_outcome("engineering_planning", CeremonyOutcome::StepFailed);
        recorder.record_ceremony_step("engineering_planning", "design", StepStatus::Failed);
        recorder.record_ceremony_transition_blocked("engineering_planning", "review");

        let text = recorder.render();
        assert!(text.contains(
            "made_ceremony_completed_total{ceremony=\"engineering_planning\",outcome=\"step_failed\"} 1"
        ));
        assert!(text.contains("status=\"failed\""));
        assert!(text.contains(
            "made_ceremony_transition_blocked_total{ceremony=\"engineering_planning\",from_state=\"review\"} 1"
        ));
    }

    #[test]
    fn records_discrimination_by_specialty_and_result() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_discrimination(&specialty(), Discrimination::Reranked);
        recorder.record_discrimination(&specialty(), Discrimination::Reranked);
        recorder.record_discrimination(&specialty(), Discrimination::Agreed);

        let text = recorder.render();
        assert!(text.contains(
            "made_judge_discrimination_total{result=\"reranked\",specialty=\"reviewer\"} 2"
        ));
        assert!(text.contains(
            "made_judge_discrimination_total{result=\"agreed\",specialty=\"reviewer\"} 1"
        ));
    }

    #[test]
    fn provider_in_flight_gauge_tracks_inc_and_dec() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.inc_provider_in_flight("vllm");
        recorder.inc_provider_in_flight("vllm");
        recorder.dec_provider_in_flight("vllm");
        recorder.observe_provider_request("vllm", "generate", DurationMs::from_millis(2_000));

        let text = recorder.render();
        // Two starts, one finish -> depth 1.
        assert!(text.contains("made_provider_in_flight{provider=\"vllm\"} 1"));
        assert!(text.contains(
            "made_provider_request_duration_seconds_sum{operation=\"generate\",provider=\"vllm\"} 2"
        ));
    }

    #[test]
    fn records_tokens_split_by_type_and_summed() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_provider_tokens("vllm", TokenUsage::new(100, 40));
        recorder.record_provider_tokens("vllm", TokenUsage::new(50, 10));
        recorder.record_judge_tokens("gemma", TokenUsage::new(200, 8));

        let text = recorder.render();
        // Counters accumulate the token counts (100+50 prompt, 40+10 completion).
        assert!(text
            .contains("made_provider_tokens_total{provider=\"vllm\",token_type=\"prompt\"} 150"));
        assert!(text.contains(
            "made_provider_tokens_total{provider=\"vllm\",token_type=\"completion\"} 50"
        ));
        assert!(text.contains("made_judge_tokens_total{model=\"gemma\",token_type=\"prompt\"} 200"));
    }

    #[test]
    fn records_provider_errors_by_provider_and_kind() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_provider_error("vllm", LlmErrorKind::RateLimited);
        recorder.record_provider_error("vllm", LlmErrorKind::Timeout);
        recorder.record_provider_error("openai", LlmErrorKind::Unauthorized);

        let text = recorder.render();
        assert!(text.contains(
            "made_provider_errors_total{error_kind=\"rate_limited\",provider=\"vllm\"} 1"
        ));
        assert!(
            text.contains("made_provider_errors_total{error_kind=\"timeout\",provider=\"vllm\"} 1")
        );
        assert!(text.contains(
            "made_provider_errors_total{error_kind=\"unauthorized\",provider=\"openai\"} 1"
        ));
    }

    #[test]
    fn records_judge_errors_by_model_and_kind() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_judge_error("gemma", LlmErrorKind::Timeout);
        recorder.record_judge_error("gemma", LlmErrorKind::Timeout);
        recorder.record_judge_error("gemma", LlmErrorKind::RateLimited);

        let text = recorder.render();
        assert!(text.contains("made_judge_errors_total{error_kind=\"timeout\",model=\"gemma\"} 2"));
        assert!(
            text.contains("made_judge_errors_total{error_kind=\"rate_limited\",model=\"gemma\"} 1")
        );
    }

    #[test]
    fn observes_judge_latency_in_seconds_and_score_by_model() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.observe_judge_latency("gemma", DurationMs::from_millis(45_000));
        recorder.observe_judge_score("gemma", Score::new(0.9).unwrap());

        let text = recorder.render();
        assert!(text.contains("made_judge_latency_seconds_sum{model=\"gemma\"} 45"));
        assert!(text.contains("made_judge_score_sum{model=\"gemma\"} 0.9"));
        assert!(text.contains("made_judge_score_count{model=\"gemma\"} 1"));
    }

    #[test]
    fn records_outcomes_with_specialty_and_outcome_labels() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::Success);
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::Success);
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::NoValidProposal);

        let text = recorder.render();
        assert!(text.contains(
            "made_deliberation_completed_total{outcome=\"success\",specialty=\"reviewer\"} 2"
        ));
        assert!(text.contains(
            "made_deliberation_completed_total{outcome=\"no_valid_proposal\",specialty=\"reviewer\"} 1"
        ));
    }

    #[test]
    fn observes_duration_in_seconds_and_winner_score() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.observe_deliberation_duration(&specialty(), DurationMs::from_millis(2_500));
        recorder.observe_winner_score(&specialty(), Score::new(0.75).unwrap());

        let text = recorder.render();
        // 2500ms == 2.5s — lands in the histogram's running sum.
        assert!(text.contains("made_deliberation_duration_seconds_sum{specialty=\"reviewer\"} 2.5"));
        assert!(text.contains("made_deliberation_winner_score_sum{specialty=\"reviewer\"} 0.75"));
        assert!(text.contains("made_deliberation_winner_score_count{specialty=\"reviewer\"} 1"));
    }
}
