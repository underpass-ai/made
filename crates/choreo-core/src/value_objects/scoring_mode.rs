//! [`ScoringMode`] — how a judge-aware scorer derived a proposal's score.
//!
//! The signal behind the scoring-mode-integrity metric. When an LLM judge
//! is wired but a proposal's score still comes from the pass-fraction
//! fallback, the judge silently did not score it — a misconfiguration the
//! service keeps "working" through. Counting the two modes makes that
//! visible.

/// Which branch a judge-aware scoring policy took for one proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScoringMode {
    /// The judge's verdict was present and used as the score.
    JudgeVerdict,
    /// No judge verdict was present; the score fell back to the structural
    /// pass-fraction.
    UniformFallback,
}

impl ScoringMode {
    /// Stable, low-cardinality label value for metrics exposition.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::JudgeVerdict => "judge_verdict",
            Self::UniformFallback => "uniform_fallback",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_distinct_and_stable() {
        assert_eq!(ScoringMode::JudgeVerdict.as_label(), "judge_verdict");
        assert_eq!(ScoringMode::UniformFallback.as_label(), "uniform_fallback");
    }
}
