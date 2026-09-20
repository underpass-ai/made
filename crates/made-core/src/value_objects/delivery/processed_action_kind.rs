use std::fmt;

use serde::{Deserialize, Serialize};

/// What the host did about a delivery, once it had acted.
///
/// The ledger records which kind of act closed a delivery, not the act
/// itself: the act happened through the engine's own commands, under
/// their own authorization. This is the receipt, not the permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessedActionKind {
    /// The host answered a supervisor's question.
    Responded,
    /// The host handed the work to an agent.
    Delegated,
    /// The host took up a result and carried it into the ceremony.
    Integrated,
    /// The host sent the work back for correction.
    CorrectionRequested,
    /// The host proposed a transition.
    TransitionProposed,
    /// The host put the question to a person.
    EscalatedToUser,
    /// The host looked and decided nothing was needed.
    NoAction,
}

impl ProcessedActionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Responded => "responded",
            Self::Delegated => "delegated",
            Self::Integrated => "integrated",
            Self::CorrectionRequested => "correction_requested",
            Self::TransitionProposed => "transition_proposed",
            Self::EscalatedToUser => "escalated_to_user",
            Self::NoAction => "no_action",
        }
    }
}

impl fmt::Display for ProcessedActionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
