//! Scoring adapters.
//!
//! The domain defines `ScoringPort` as a single trait that aggregates
//! validator reports into a [`Score`]. This module ships one minimal,
//! honestly-described implementation; operators can plug in their own
//! policy (weighted average, fail-fast, learned combinator, …) by
//! implementing `ScoringPort` elsewhere.

use async_trait::async_trait;
use choreo_core::entities::ValidatorReport;
use choreo_core::error::DomainError;
use choreo_core::ports::ScoringPort;
use choreo_core::value_objects::Score;
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
#[derive(Debug, Default, Clone, Copy)]
pub struct JudgeAwareScoring;

impl JudgeAwareScoring {
    #[must_use]
    pub const fn new() -> Self {
        Self
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
            return Score::new(verdict.clamp(0.0, 1.0));
        }
        if reports.is_empty() {
            return Ok(Score::MIN);
        }
        let passed = reports.iter().filter(|report| report.passed()).count();
        #[allow(clippy::cast_precision_loss)]
        let value = passed as f64 / reports.len() as f64;
        Score::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::value_objects::Attributes;
    use serde_json::json;
    use std::collections::BTreeMap;

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
}
