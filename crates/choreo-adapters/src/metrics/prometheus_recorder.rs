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

use choreo_core::error::DomainError;
use choreo_core::ports::MetricsRecorderPort;
use choreo_core::value_objects::{
    DeliberationOutcome, DurationMs, LlmErrorKind, Score, Specialty, TokenUsage,
};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
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
}

impl PrometheusMetricsRecorder {
    /// Build the recorder, defining and registering every metric family.
    ///
    /// # Errors
    ///
    /// Returns a [`DomainError::InvariantViolated`] if a metric fails to
    /// register — a duplicate name or malformed label is a wiring bug,
    /// surfaced at composition time rather than silently at runtime.
    pub fn new() -> Result<Self, DomainError> {
        let registry = Registry::new();
        let deliberation_duration_seconds = register_histogram(
            &registry,
            "choreo_deliberation_duration_seconds",
            "End-to-end wall-clock duration of a deliberation that ran to completion.",
            DURATION_BUCKETS_SECONDS,
            &["specialty"],
        )?;
        let deliberation_winner_score = register_histogram(
            &registry,
            "choreo_deliberation_winner_score",
            "Score of the winning proposal of a deliberation.",
            SCORE_BUCKETS,
            &["specialty"],
        )?;
        let deliberation_completed_total = register_counter(
            &registry,
            "choreo_deliberation_completed_total",
            "Deliberations that ran to completion, partitioned by terminal outcome.",
            &["specialty", "outcome"],
        )?;
        let judge_latency_seconds = register_histogram(
            &registry,
            "choreo_judge_latency_seconds",
            "Latency of a single LLM-judge rating call, success or failure.",
            JUDGE_LATENCY_BUCKETS_SECONDS,
            &["model"],
        )?;
        let judge_score = register_histogram(
            &registry,
            "choreo_judge_score",
            "Distribution of the LLM judge's 0.0-1.0 verdicts.",
            SCORE_BUCKETS,
            &["model"],
        )?;
        let judge_errors_total = register_counter(
            &registry,
            "choreo_judge_errors_total",
            "Failed LLM-judge calls, partitioned by error classification.",
            &["model", "error_kind"],
        )?;
        let provider_errors_total = register_counter(
            &registry,
            "choreo_provider_errors_total",
            "Failed proposing-agent calls, partitioned by provider and error classification.",
            &["provider", "error_kind"],
        )?;
        let judge_tokens_total = register_counter(
            &registry,
            "choreo_judge_tokens_total",
            "Tokens consumed by LLM-judge calls, partitioned by token type.",
            &["model", "token_type"],
        )?;
        let provider_tokens_total = register_counter(
            &registry,
            "choreo_provider_tokens_total",
            "Tokens consumed by proposing-agent calls, partitioned by provider and token type.",
            &["provider", "token_type"],
        )?;
        let provider_request_duration_seconds = register_histogram(
            &registry,
            "choreo_provider_request_duration_seconds",
            "Latency of a single proposing-agent call, by provider and operation.",
            PROVIDER_LATENCY_BUCKETS_SECONDS,
            &["provider", "operation"],
        )?;
        let provider_in_flight = register_gauge(
            &registry,
            "choreo_provider_in_flight",
            "In-flight proposing-agent calls per provider; the vLLM serial-saturation signal.",
            &["provider"],
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

        let text = recorder.render();
        assert!(text.contains("# TYPE choreo_deliberation_duration_seconds histogram"));
        assert!(text.contains("# TYPE choreo_deliberation_winner_score histogram"));
        assert!(text.contains("# TYPE choreo_deliberation_completed_total counter"));
        assert!(text.contains("# TYPE choreo_judge_latency_seconds histogram"));
        assert!(text.contains("# TYPE choreo_judge_score histogram"));
        assert!(text.contains("# TYPE choreo_judge_errors_total counter"));
        assert!(text.contains("# TYPE choreo_provider_errors_total counter"));
        assert!(text.contains("# TYPE choreo_judge_tokens_total counter"));
        assert!(text.contains("# TYPE choreo_provider_tokens_total counter"));
        assert!(text.contains("# TYPE choreo_provider_request_duration_seconds histogram"));
        assert!(text.contains("# TYPE choreo_provider_in_flight gauge"));
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
        assert!(text.contains("choreo_provider_in_flight{provider=\"vllm\"} 1"));
        assert!(text.contains(
            "choreo_provider_request_duration_seconds_sum{operation=\"generate\",provider=\"vllm\"} 2"
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
            .contains("choreo_provider_tokens_total{provider=\"vllm\",token_type=\"prompt\"} 150"));
        assert!(text.contains(
            "choreo_provider_tokens_total{provider=\"vllm\",token_type=\"completion\"} 50"
        ));
        assert!(
            text.contains("choreo_judge_tokens_total{model=\"gemma\",token_type=\"prompt\"} 200")
        );
    }

    #[test]
    fn records_provider_errors_by_provider_and_kind() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_provider_error("vllm", LlmErrorKind::RateLimited);
        recorder.record_provider_error("vllm", LlmErrorKind::Timeout);
        recorder.record_provider_error("openai", LlmErrorKind::Unauthorized);

        let text = recorder.render();
        assert!(text.contains(
            "choreo_provider_errors_total{error_kind=\"rate_limited\",provider=\"vllm\"} 1"
        ));
        assert!(text
            .contains("choreo_provider_errors_total{error_kind=\"timeout\",provider=\"vllm\"} 1"));
        assert!(text.contains(
            "choreo_provider_errors_total{error_kind=\"unauthorized\",provider=\"openai\"} 1"
        ));
    }

    #[test]
    fn records_judge_errors_by_model_and_kind() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_judge_error("gemma", LlmErrorKind::Timeout);
        recorder.record_judge_error("gemma", LlmErrorKind::Timeout);
        recorder.record_judge_error("gemma", LlmErrorKind::RateLimited);

        let text = recorder.render();
        assert!(
            text.contains("choreo_judge_errors_total{error_kind=\"timeout\",model=\"gemma\"} 2")
        );
        assert!(text
            .contains("choreo_judge_errors_total{error_kind=\"rate_limited\",model=\"gemma\"} 1"));
    }

    #[test]
    fn observes_judge_latency_in_seconds_and_score_by_model() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.observe_judge_latency("gemma", DurationMs::from_millis(45_000));
        recorder.observe_judge_score("gemma", Score::new(0.9).unwrap());

        let text = recorder.render();
        assert!(text.contains("choreo_judge_latency_seconds_sum{model=\"gemma\"} 45"));
        assert!(text.contains("choreo_judge_score_sum{model=\"gemma\"} 0.9"));
        assert!(text.contains("choreo_judge_score_count{model=\"gemma\"} 1"));
    }

    #[test]
    fn records_outcomes_with_specialty_and_outcome_labels() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::Success);
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::Success);
        recorder.record_deliberation_outcome(&specialty(), DeliberationOutcome::NoValidProposal);

        let text = recorder.render();
        assert!(text.contains(
            "choreo_deliberation_completed_total{outcome=\"success\",specialty=\"reviewer\"} 2"
        ));
        assert!(text.contains(
            "choreo_deliberation_completed_total{outcome=\"no_valid_proposal\",specialty=\"reviewer\"} 1"
        ));
    }

    #[test]
    fn observes_duration_in_seconds_and_winner_score() {
        let recorder = PrometheusMetricsRecorder::new().unwrap();
        recorder.observe_deliberation_duration(&specialty(), DurationMs::from_millis(2_500));
        recorder.observe_winner_score(&specialty(), Score::new(0.75).unwrap());

        let text = recorder.render();
        // 2500ms == 2.5s — lands in the histogram's running sum.
        assert!(
            text.contains("choreo_deliberation_duration_seconds_sum{specialty=\"reviewer\"} 2.5")
        );
        assert!(text.contains("choreo_deliberation_winner_score_sum{specialty=\"reviewer\"} 0.75"));
        assert!(text.contains("choreo_deliberation_winner_score_count{specialty=\"reviewer\"} 1"));
    }
}
