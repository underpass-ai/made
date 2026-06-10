//! [`DeliberationOutcome`] — the terminal result of a deliberation run.

/// The terminal result of a deliberation that ran to completion.
///
/// A deliberation that reaches its end lands in exactly one of these:
/// either a winning proposal was selected, or — under an output
/// contract — every proposal was generated but none satisfied it.
///
/// Adapter-shaped aborts (an agent or validator returning `Err`) are
/// deliberately **not** outcomes: they surface as [`DomainError`]s and,
/// in later observability slices, as dedicated error counters. Keeping
/// this enum to the two genuine end-states keeps the `outcome` metric
/// label low-cardinality and meaningful.
///
/// [`DomainError`]: crate::error::DomainError
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliberationOutcome {
    /// A winning proposal was selected and returned to the caller.
    Success,
    /// An output contract was in force but no proposal satisfied it.
    NoValidProposal,
}

impl DeliberationOutcome {
    /// Stable, low-cardinality label value for metrics exposition.
    ///
    /// The strings are part of the metric contract; downstream
    /// dashboards and alerts match on them, so they must not change.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NoValidProposal => "no_valid_proposal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_stable_and_distinct() {
        assert_eq!(DeliberationOutcome::Success.as_label(), "success");
        assert_eq!(
            DeliberationOutcome::NoValidProposal.as_label(),
            "no_valid_proposal"
        );
        assert_ne!(
            DeliberationOutcome::Success.as_label(),
            DeliberationOutcome::NoValidProposal.as_label()
        );
    }
}
