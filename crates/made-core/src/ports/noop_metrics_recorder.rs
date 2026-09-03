use crate::ports::MetricsRecorderPort;
use crate::value_objects::{
    CeremonyOutcome, DeliberationOutcome, Discrimination, DurationMs, LlmErrorKind, Score,
    ScoringMode, Specialty, StepStatus, TokenUsage,
};

/// Metrics sink that intentionally discards every observation.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMetricsRecorder;

impl MetricsRecorderPort for NoopMetricsRecorder {
    fn observe_deliberation_duration(&self, _: &Specialty, _: DurationMs) {}
    fn record_deliberation_outcome(&self, _: &Specialty, _: DeliberationOutcome) {}
    fn observe_winner_score(&self, _: &Specialty, _: Score) {}
    fn observe_judge_latency(&self, _: &str, _: DurationMs) {}
    fn observe_judge_score(&self, _: &str, _: Score) {}
    fn record_judge_error(&self, _: &str, _: LlmErrorKind) {}
    fn record_provider_error(&self, _: &str, _: LlmErrorKind) {}
    fn record_judge_tokens(&self, _: &str, _: TokenUsage) {}
    fn record_provider_tokens(&self, _: &str, _: TokenUsage) {}
    fn observe_provider_request(&self, _: &str, _: &str, _: DurationMs) {}
    fn inc_provider_in_flight(&self, _: &str) {}
    fn dec_provider_in_flight(&self, _: &str) {}
    fn record_discrimination(&self, _: &Specialty, _: Discrimination) {}
    fn record_ceremony_outcome(&self, _: &str, _: CeremonyOutcome) {}
    fn observe_ceremony_duration(&self, _: &str, _: DurationMs) {}
    fn observe_ceremony_step_duration(&self, _: &str, _: &str, _: DurationMs) {}
    fn record_ceremony_step(&self, _: &str, _: &str, _: StepStatus) {}
    fn record_ceremony_transition_blocked(&self, _: &str, _: &str) {}
    fn observe_nats_publish(&self, _: &str, _: DurationMs) {}
    fn record_nats_publish_error(&self, _: &str, _: &str) {}
    fn set_postgres_pool_in_use(&self, _: i64) {}
    fn record_scoring_mode(&self, _: ScoringMode) {}
}
