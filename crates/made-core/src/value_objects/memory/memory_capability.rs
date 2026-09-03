use serde::{Deserialize, Serialize};

/// One capability advertised by a memory backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCapability {
    Remembering,
    Recalling,
    AnsweringQuestions,
    TravellingInTime,
    KeepingEvidence,
    KeepingReasons,
    FollowingReasons,
}

impl MemoryCapability {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Remembering => "remembering",
            Self::Recalling => "recalling",
            Self::AnsweringQuestions => "answering_questions",
            Self::TravellingInTime => "travelling_in_time",
            Self::KeepingEvidence => "keeping_evidence",
            Self::KeepingReasons => "keeping_reasons",
            Self::FollowingReasons => "following_reasons",
        }
    }
}
