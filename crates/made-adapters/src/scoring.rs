//! Scoring adapters.
//!
//! The domain defines `ScoringPort` as a single trait that aggregates
//! validator reports into a [`Score`]. This module ships one minimal,
//! honestly-described implementation; operators can plug in their own
//! policy (weighted average, fail-fast, learned combinator, …) by
//! implementing `ScoringPort` elsewhere.

mod judge_aware_scoring;
mod uniform_scoring;

pub use judge_aware_scoring::JudgeAwareScoring;
pub use uniform_scoring::UniformScoring;

/// Details key under which an LLM judge writes its 0.0–1.0 verdict. This
/// is the contract between [`crate::agents::judge::LlmJudgeValidator`]
/// (writer) and [`JudgeAwareScoring`] (reader).
pub const JUDGE_SCORE_DETAIL_KEY: &str = "judge.score";

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::{Proposal, RankedOutcome, ValidationOutcome, ValidatorReport};
    use made_core::ports::ScoringPort;
    use made_core::value_objects::{
        AgentId, Attributes, Discrimination, ProposalId, Score, Specialty,
    };
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
