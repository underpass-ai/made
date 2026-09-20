use std::fmt;

use serde::{Deserialize, Serialize};

/// Why a ceremony is worth the integrator's attention.
///
/// A closed list, because the loop's stopping rules are written over it:
/// a kind the engine cannot name is a kind an integrator cannot be told
/// to stop for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// A step produced a result the integrator can take up.
    ResultAvailable,
    /// A completed step's own output says it was not accepted.
    ReviewRejected,
    /// A step failed or ran out of time.
    StepFailed,
    /// The ceremony cannot move without something outside it.
    Blocked,
    /// A human guard is waiting for an answer.
    HumanDecisionRequested,
    /// A ceremony or state deadline passed.
    DeadlineExceeded,
    /// Nothing has happened for longer than the policy tolerates.
    InactivityDetected,
    /// A supervisor asked a question of whoever is working.
    InterventionRequested,
    /// The ceremony reached a terminal phase.
    CeremonyEnded,
}

impl AttentionKind {
    /// Every kind, which is also the default policy's selection.
    pub const ALL: [Self; 9] = [
        Self::ResultAvailable,
        Self::ReviewRejected,
        Self::StepFailed,
        Self::Blocked,
        Self::HumanDecisionRequested,
        Self::DeadlineExceeded,
        Self::InactivityDetected,
        Self::InterventionRequested,
        Self::CeremonyEnded,
    ];

    /// The kind a derived identity ends with, when it ends with one.
    ///
    /// [`AttentionEventId::derive`](super::AttentionEventId::derive)
    /// spells the kind into the identity, and the ledger holds that
    /// identity rather than the event. Reading it back is what lets a
    /// queue be weighed — which of the things waiting may be dropped —
    /// without keeping a second copy of what each delivery is about.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == label)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResultAvailable => "result_available",
            Self::ReviewRejected => "review_rejected",
            Self::StepFailed => "step_failed",
            Self::Blocked => "blocked",
            Self::HumanDecisionRequested => "human_decision_requested",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::InactivityDetected => "inactivity_detected",
            Self::InterventionRequested => "intervention_requested",
            Self::CeremonyEnded => "ceremony_ended",
        }
    }

    /// Whether dropping this kind under overflow would lose a decision.
    ///
    /// Overflow discards the oldest item that nobody is waiting on. A
    /// human decision, a block or the end of a ceremony are things the
    /// loop stops for, so they are never the ones discarded.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(
            self,
            Self::Blocked
                | Self::HumanDecisionRequested
                | Self::CeremonyEnded
                | Self::DeadlineExceeded
                | Self::InterventionRequested
        )
    }
}

impl fmt::Display for AttentionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_listed_once_and_serialises_as_its_name() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in AttentionKind::ALL {
            assert!(seen.insert(kind), "{kind} is listed twice");
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{}\"", kind.as_str())
            );
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn results_and_inactivity_are_the_droppable_kinds() {
        assert!(!AttentionKind::ResultAvailable.is_blocking());
        assert!(!AttentionKind::InactivityDetected.is_blocking());
        assert!(AttentionKind::HumanDecisionRequested.is_blocking());
    }
}
