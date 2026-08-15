//! Scoring adapters.
//!
//! The domain defines `ScoringPort` as a single trait that aggregates
//! validator reports into a [`Score`]. This module ships one minimal,
//! honestly-described implementation; operators can plug in their own
//! policy (weighted average, fail-fast, learned combinator, …) by
//! implementing `ScoringPort` elsewhere.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{RankedOutcome, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::{MetricsRecorderPort, NoopMetricsRecorder, ScoringPort};
use made_core::value_objects::{Discrimination, ProposalId, Score, ScoringMode};
use serde_json::Value;

/// Details key under which an LLM judge writes its 0.0–1.0 verdict. This
/// is the contract between [`crate::agents::judge::LlmJudgeValidator`]
/// (writer) and [`JudgeAwareScoring`] (reader).
pub const JUDGE_SCORE_DETAIL_KEY: &str = "judge.score";

/// Uniform scoring: the score is the fraction of reports that passed.
///
/// With zero reports the score is [`Score::MIN`] — no evidence means
/// no confidence. That choice biases operators to configure at least
/// one validator rather than silently returning a perfect score from
/// thin air.
#[derive(Debug, Default, Clone, Copy)]
pub struct UniformScoring;

impl UniformScoring {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ScoringPort for UniformScoring {
    async fn score(&self, reports: &[ValidatorReport]) -> Result<Score, DomainError> {
        if reports.is_empty() {
            return Ok(Score::MIN);
        }
        let passed = reports.iter().filter(|r| r.passed()).count();
        let total = reports.len();
        #[allow(clippy::cast_precision_loss)]
        let value = passed as f64 / total as f64;
        Score::new(value)
    }
}

/// Scoring that lets an LLM judge decide the ranking.
///
/// If any report carries a numeric verdict under
/// [`JUDGE_SCORE_DETAIL_KEY`] (written by `LlmJudgeValidator`), that
/// value *is* the proposal's score — so the strongest proposal wins,
/// not an arbitrary one among those that merely passed the structural
/// validators. When no judge ran, it falls back to the same
/// pass-fraction policy as [`UniformScoring`], so it is a safe default.
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
        // No judge verdict on this proposal: the score (empty -> MIN, or
        // pass-fraction) is the uniform fallback.
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

    /// Whether the judge's verdict changed the winner. Compares the
    /// score-ranked top (the judge's pick) against the *structural
    /// baseline*: the proposal a judge-free ranking would choose — the
    /// smallest-id proposal among those passing every non-judge report,
    /// which is exactly the arbitrary id tie-break the judge replaces.
    ///
    /// Returns `None` when no judge actually scored (so the metric stays
    /// judge-specific) or when there are fewer than two proposals (nothing
    /// to discriminate among).
    fn discrimination(&self, ranked: &[RankedOutcome]) -> Option<Discrimination> {
        if ranked.len() < 2 || !ranked.iter().any(has_judge_verdict) {
            return None;
        }
        // A shared top score means the judge did not separate the leaders.
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

/// Whether a ranked outcome carries the LLM judge's verdict.
fn has_judge_verdict(outcome: &RankedOutcome) -> bool {
    outcome.outcome().reports().iter().any(is_judge_report)
}

/// A report is the judge's iff it carries the judge-score detail key —
/// the contract between the judge validator and this scorer.
fn is_judge_report(report: &ValidatorReport) -> bool {
    report.details().get(JUDGE_SCORE_DETAIL_KEY).is_some()
}

/// The proposal a judge-free ranking would pick: the smallest-id proposal
/// among those passing every structural (non-judge) report. Falls back to
/// the smallest id overall when none pass the structural validators.
fn structural_baseline_winner(ranked: &[RankedOutcome]) -> Option<&ProposalId> {
    ranked
        .iter()
        .filter(|outcome| structurally_valid(outcome))
        .map(|outcome| outcome.proposal().id())
        .min()
        .or_else(|| ranked.iter().map(|outcome| outcome.proposal().id()).min())
}

/// Whether every non-judge report on this proposal passed.
fn structurally_valid(outcome: &RankedOutcome) -> bool {
    outcome
        .outcome()
        .reports()
        .iter()
        .filter(|report| !is_judge_report(report))
        .all(ValidatorReport::passed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::{Proposal, ValidationOutcome};
    use made_core::value_objects::{AgentId, Attributes, Specialty};
    use serde_json::json;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn report(passed: bool) -> ValidatorReport {
        ValidatorReport::new("k", passed, "", Attributes::empty()).unwrap()
    }

    fn judged(score: f64) -> ValidatorReport {
        let details = Attributes::new(BTreeMap::from([(
            JUDGE_SCORE_DETAIL_KEY.to_owned(),
            json!(score),
        )]))
        .unwrap();
        ValidatorReport::new("llm_judge", score >= 0.5, "", details).unwrap()
    }

    /// Build a ranked outcome for `id` carrying the given final `score`,
    /// an optional judge verdict, and whether its structural validator
    /// passed. Callers pass these in score-descending order.
    fn ranked(id: &str, score: f64, judge: Option<f64>, structural_pass: bool) -> RankedOutcome {
        let mut reports = vec![report(structural_pass)];
        if let Some(verdict) = judge {
            reports.push(judged(verdict));
        }
        let proposal = Proposal::new(
            ProposalId::new(id).unwrap(),
            AgentId::new("agent").unwrap(),
            Specialty::new("reviewer").unwrap(),
            "content".to_owned(),
            Attributes::empty(),
            datetime!(2026-04-15 12:00:00 UTC),
        )
        .unwrap();
        let outcome = ValidationOutcome::new(Score::new(score).unwrap(), reports);
        RankedOutcome::new(proposal, outcome, 0)
    }

    #[tokio::test]
    async fn judge_aware_uses_the_judge_verdict_when_present() {
        let s = JudgeAwareScoring::new()
            .score(&[report(true), judged(0.82)])
            .await
            .unwrap();
        assert!((s.get() - 0.82).abs() < 1e-9);
    }

    #[tokio::test]
    async fn judge_aware_clamps_out_of_range_verdicts() {
        let s = JudgeAwareScoring::new()
            .score(&[judged(1.5)])
            .await
            .unwrap();
        assert_eq!(s, Score::MAX);
    }

    #[tokio::test]
    async fn judge_aware_falls_back_to_pass_fraction_without_a_judge() {
        let s = JudgeAwareScoring::new()
            .score(&[report(true), report(false)])
            .await
            .unwrap();
        assert_eq!(s.get(), 0.5);
    }

    #[tokio::test]
    async fn judge_aware_empty_reports_score_to_min() {
        let s = JudgeAwareScoring::new().score(&[]).await.unwrap();
        assert_eq!(s, Score::MIN);
    }

    #[tokio::test]
    async fn empty_reports_score_to_min() {
        let s = UniformScoring::new().score(&[]).await.unwrap();
        assert_eq!(s, Score::MIN);
    }

    #[tokio::test]
    async fn all_passed_scores_to_max() {
        let s = UniformScoring::new()
            .score(&[report(true), report(true)])
            .await
            .unwrap();
        assert_eq!(s, Score::MAX);
    }

    #[tokio::test]
    async fn mixed_reports_score_to_pass_fraction() {
        let s = UniformScoring::new()
            .score(&[report(true), report(false), report(true), report(false)])
            .await
            .unwrap();
        assert_eq!(s.get(), 0.5);
    }

    #[tokio::test]
    async fn all_failed_scores_to_min() {
        let s = UniformScoring::new()
            .score(&[report(false), report(false)])
            .await
            .unwrap();
        assert_eq!(s, Score::MIN);
    }

    // --- discrimination ---------------------------------------------------

    #[test]
    fn discrimination_is_reranked_when_judge_top_is_not_the_id_baseline() {
        // Judge ranks "b" (score 0.9) over "a" (0.5); both structurally
        // valid, so the id-baseline winner is "a". The judge reranked.
        let ranking = [
            ranked("b", 0.9, Some(0.9), true),
            ranked("a", 0.5, Some(0.5), true),
        ];
        assert_eq!(
            JudgeAwareScoring::new().discrimination(&ranking),
            Some(Discrimination::Reranked)
        );
    }

    #[test]
    fn discrimination_is_agreed_when_judge_top_is_the_id_baseline() {
        // Judge's top "a" is also the smallest-id structurally-valid pick.
        let ranking = [
            ranked("a", 0.9, Some(0.9), true),
            ranked("b", 0.5, Some(0.5), true),
        ];
        assert_eq!(
            JudgeAwareScoring::new().discrimination(&ranking),
            Some(Discrimination::Agreed)
        );
    }

    #[test]
    fn discrimination_is_tie_when_top_score_is_shared() {
        let ranking = [
            ranked("a", 0.8, Some(0.8), true),
            ranked("b", 0.8, Some(0.8), true),
        ];
        assert_eq!(
            JudgeAwareScoring::new().discrimination(&ranking),
            Some(Discrimination::Tie)
        );
    }

    #[test]
    fn discrimination_reranks_past_a_structurally_invalid_baseline() {
        // The judge's top "b" is structurally invalid; the only valid
        // proposal is "a", so the baseline is "a" and the judge reranked.
        let ranking = [
            ranked("b", 0.9, Some(0.9), false),
            ranked("a", 0.5, Some(0.5), true),
        ];
        assert_eq!(
            JudgeAwareScoring::new().discrimination(&ranking),
            Some(Discrimination::Reranked)
        );
    }

    #[test]
    fn discrimination_is_none_without_a_judge_verdict() {
        let ranking = [ranked("a", 1.0, None, true), ranked("b", 0.5, None, true)];
        assert_eq!(JudgeAwareScoring::new().discrimination(&ranking), None);
    }

    #[test]
    fn discrimination_is_none_with_a_single_proposal() {
        let ranking = [ranked("a", 0.9, Some(0.9), true)];
        assert_eq!(JudgeAwareScoring::new().discrimination(&ranking), None);
    }

    #[test]
    fn uniform_scoring_never_reports_discrimination() {
        let ranking = [
            ranked("b", 0.9, Some(0.9), true),
            ranked("a", 0.5, Some(0.5), true),
        ];
        assert_eq!(UniformScoring::new().discrimination(&ranking), None);
    }
}
