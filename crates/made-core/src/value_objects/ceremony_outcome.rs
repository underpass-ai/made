//! [`CeremonyOutcome`] — the terminal result of a ceremony run.
//!
//! A ceremony driven to a stop ends in exactly one of these. Unlike a
//! step's [`StepStatus`](super::StepStatus), this names *why the whole
//! ceremony stopped*: it completed, a step failed, no transition could
//! fire, the transition safety cap was hit, a step exhausted its semantic
//! repeat limit, or the instance already existed. Adapter-shaped aborts
//! (persistence/transport errors) are not
//! outcomes — they surface as errors, not as a recorded end-state.

/// How a ceremony run terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CeremonyOutcome {
    /// Reached a terminal state — the ceremony finished.
    Completed,
    /// A step did not complete successfully and aborted the run.
    StepFailed,
    /// No transition out of the current state was satisfiable (a guard
    /// deadlock or a missing event).
    NoTransition,
    /// The transition safety cap was hit without reaching a terminal
    /// state.
    IterationLimit,
    /// A bounded semantic step repeat used every permitted iteration without
    /// satisfying its declared stop condition.
    RepeatLimit,
    /// An instance with the same id already existed; the run was rejected.
    AlreadyExists,
}

impl CeremonyOutcome {
    /// Stable, low-cardinality label value for metrics exposition. Part of
    /// the metric contract; dashboards and alerts match on it.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::StepFailed => "step_failed",
            Self::NoTransition => "no_transition",
            Self::IterationLimit => "iteration_limit",
            Self::RepeatLimit => "repeat_limit",
            Self::AlreadyExists => "already_exists",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_distinct_and_stable() {
        let all = [
            CeremonyOutcome::Completed,
            CeremonyOutcome::StepFailed,
            CeremonyOutcome::NoTransition,
            CeremonyOutcome::IterationLimit,
            CeremonyOutcome::RepeatLimit,
            CeremonyOutcome::AlreadyExists,
        ];
        let labels: std::collections::BTreeSet<&str> =
            all.iter().map(|outcome| outcome.as_label()).collect();
        assert_eq!(labels.len(), all.len());
    }
}
