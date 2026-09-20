use std::fmt;

use serde::{Deserialize, Serialize};

/// What a supervisor wants back from the agent it interrupted.
///
/// Separate from [`CeremonyInterventionKind`], which says what sort of
/// work the item is. The intent says what the interruption is *for*, so
/// a host reading a delivery can tell an open question from a standing
/// constraint without reading the prose and guessing.
///
/// [`CeremonyInterventionKind`]: super::CeremonyInterventionKind
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyInterventionIntent {
    /// An answer is wanted before the work goes on.
    Question,
    /// An observation on work already done; no answer is required.
    Feedback,
    /// A rule the rest of the work has to respect.
    Constraint,
    /// A pause to confirm the work is still going the right way.
    Checkpoint,
}

impl CeremonyInterventionIntent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Feedback => "feedback",
            Self::Constraint => "constraint",
            Self::Checkpoint => "checkpoint",
        }
    }

    /// Whether leaving this unanswered leaves something unresolved.
    ///
    /// Feedback and a constraint are told, not asked: a host that has
    /// taken them has finished with them. A question and a checkpoint
    /// are open until somebody answers.
    #[must_use]
    pub const fn expects_response(self) -> bool {
        matches!(self, Self::Question | Self::Checkpoint)
    }
}

impl fmt::Display for CeremonyInterventionIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_intents_that_ask_expect_an_answer() {
        assert!(CeremonyInterventionIntent::Question.expects_response());
        assert!(CeremonyInterventionIntent::Checkpoint.expects_response());
        assert!(!CeremonyInterventionIntent::Feedback.expects_response());
        assert!(!CeremonyInterventionIntent::Constraint.expects_response());
    }

    #[test]
    fn wire_names_are_the_labels() {
        assert_eq!(
            serde_json::to_value(CeremonyInterventionIntent::Constraint).unwrap(),
            serde_json::json!("constraint")
        );
        assert_eq!(CeremonyInterventionIntent::Feedback.to_string(), "feedback");
    }
}
