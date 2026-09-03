use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{RankedOutcome, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::{MetricsRecorderPort, NoopMetricsRecorder, ScoringPort};
use made_core::value_objects::{Discrimination, ProposalId, Score, ScoringMode};
use serde_json::Value;

use super::JUDGE_SCORE_DETAIL_KEY;

/// Scoring that lets an LLM judge decide the ranking.
///
/// If any report carries a numeric verdict under
/// [`JUDGE_SCORE_DETAIL_KEY`] (written by `LlmJudgeValidator`), that
/// value *is* the proposal's score — so the strongest proposal wins,
/// not an arbitrary one among those that merely passed the structural
/// validators. When no judge ran, it falls back to the same
/// pass-fraction policy as [`super::UniformScoring`], so it is a safe default.
#[derive(Clone)]
pub struct JudgeAwareScoring {
    metrics: Arc<dyn MetricsRecorderPort>,
}

impl std::fmt::Debug for JudgeAwareScoring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JudgeAwareScoring").finish()
    }
}

impl Default for JudgeAwareScoring {
    fn default() -> Self {
        Self::new()
    }
}

impl JudgeAwareScoring {
    #[must_use]
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(NoopMetricsRecorder),
        }
    }

    /// Attach a metrics recorder so the scoring-mode split (judge verdict
    /// vs uniform fallback) is counted. The composition root wires the
    /// real recorder; the default no-op keeps tests free of one.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = metrics;
        self
    }
}

#[async_trait]
impl ScoringPort for JudgeAwareScoring {
    async fn score(&self, reports: &[ValidatorReport]) -> Result<Score, DomainError> {
        if let Some(verdict) = reports
            .iter()
            .find_map(|report| report.details().get(JUDGE_SCORE_DETAIL_KEY))
            .and_then(Value::as_f64)
        {
            self.metrics.record_scoring_mode(ScoringMode::JudgeVerdict);
            return Score::new(verdict.clamp(0.0, 1.0));
        }
        self.metrics
            .record_scoring_mode(ScoringMode::UniformFallback);
        if reports.is_empty() {
            return Ok(Score::MIN);
        }
        let passed = reports.iter().filter(|report| report.passed()).count();
        #[allow(clippy::cast_precision_loss)]
        let value = passed as f64 / reports.len() as f64;
        Score::new(value)
    }

    fn discrimination(&self, ranked: &[RankedOutcome]) -> Option<Discrimination> {
        if ranked.len() < 2 || !ranked.iter().any(has_judge_verdict) {
            return None;
        }
        let top_score = ranked[0].outcome().score();
        if ranked
            .iter()
            .filter(|outcome| outcome.outcome().score() == top_score)
            .count()
            > 1
        {
            return Some(Discrimination::Tie);
        }
        let judge_winner = ranked[0].proposal().id();
        let baseline_winner = structural_baseline_winner(ranked)?;
        Some(if judge_winner == baseline_winner {
            Discrimination::Agreed
        } else {
            Discrimination::Reranked
        })
    }
}

fn has_judge_verdict(outcome: &RankedOutcome) -> bool {
    outcome.outcome().reports().iter().any(is_judge_report)
}

fn is_judge_report(report: &ValidatorReport) -> bool {
    report.details().get(JUDGE_SCORE_DETAIL_KEY).is_some()
}

fn structural_baseline_winner(ranked: &[RankedOutcome]) -> Option<&ProposalId> {
    ranked
        .iter()
        .filter(|outcome| structurally_valid(outcome))
        .map(|outcome| outcome.proposal().id())
        .min()
        .or_else(|| ranked.iter().map(|outcome| outcome.proposal().id()).min())
}

fn structurally_valid(outcome: &RankedOutcome) -> bool {
    outcome
        .outcome()
        .reports()
        .iter()
        .filter(|report| !is_judge_report(report))
        .all(ValidatorReport::passed)
}
