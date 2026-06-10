//! [`Discrimination`] — whether a scoring policy re-ordered the winner.
//!
//! The signal behind the judge-discrimination metric: does the policy's
//! ranking pick a *different* winner than a structural baseline would, or
//! does it merely confirm it? A policy that almost never reranks is
//! expensive dead weight; one that always reranks is doing real work.

/// How a scoring policy's winner compares to the structural baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Discrimination {
    /// The policy picked a different winner than the baseline — it
    /// changed the outcome.
    Reranked,
    /// The policy's winner is the same the baseline would pick.
    Agreed,
    /// The top score is shared by more than one proposal — the policy did
    /// not separate the leaders.
    Tie,
}

impl Discrimination {
    /// Stable, low-cardinality label value for metrics exposition. Part
    /// of the metric contract; dashboards match on it.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Reranked => "reranked",
            Self::Agreed => "agreed",
            Self::Tie => "tie",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_distinct_and_stable() {
        assert_eq!(Discrimination::Reranked.as_label(), "reranked");
        assert_eq!(Discrimination::Agreed.as_label(), "agreed");
        assert_eq!(Discrimination::Tie.as_label(), "tie");
    }
}
