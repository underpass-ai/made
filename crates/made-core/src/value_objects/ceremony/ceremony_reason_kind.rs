use serde::{Deserialize, Serialize};

use super::ReasonAsserter;

/// How one thing a session produced explains another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyReasonKind {
    /// This contribution is the reply to that agenda item.
    Answers,
    /// This was decided because of that.
    ChosenBecause,
    /// That decision is what permitted this action.
    ///
    /// The edge anyone reviewing a session afterwards looks for first:
    /// not what happened, but what made it allowed to happen. A session
    /// that records an action and cannot point at the decision behind
    /// it is a record of events rather than of authority.
    ///
    /// Only whoever made the decision may say what it authorised. That
    /// a third party can see the connection does not make it theirs to
    /// assert: attributing authorising force to somebody else's
    /// decision is the receipt this engine refuses to write.
    Authorizes,
    /// This was brought about by doing that — **the how**.
    ///
    /// The one that turns a resolved session from a precedent into a
    /// procedure. A session that records why it resolved and not how
    /// cannot be turned into anything anyone can repeat.
    AchievedBy,
    /// This came about because of that.
    FollowsFrom,
    /// This honours a limit that one set.
    SatisfiesConstraint,
    /// This breaks a limit that one set, knowingly.
    ViolatesConstraint,
    /// This replaces that as what is believed now.
    Supersedes,
    /// These cannot both be true.
    Contradicts,
}

impl CeremonyReasonKind {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Answers => "answers",
            Self::Authorizes => "authorizes",
            Self::ChosenBecause => "chosen_because",
            Self::AchievedBy => "achieved_by",
            Self::FollowsFrom => "follows_from",
            Self::SatisfiesConstraint => "satisfies_constraint",
            Self::ViolatesConstraint => "violates_constraint",
            Self::Supersedes => "supersedes",
            Self::Contradicts => "contradicts",
        }
    }

    /// Who may assert a reason of this kind.
    #[must_use]
    pub const fn asserter(self) -> ReasonAsserter {
        match self {
            // Structure. The engine sees which contribution answered
            // which item; it is not judging anything.
            Self::Answers => ReasonAsserter::TheEngine,
            // Testimony. Only whoever decided knows what decided them,
            // and only whoever acted knows how they did it.
            Self::Authorizes | Self::ChosenBecause | Self::AchievedBy => ReasonAsserter::ItsAuthor,
            // Claims about the world, open to anyone and weighable by
            // everyone. The engine is excluded from these on purpose:
            // a session ending well after an action is not the action
            // having worked, and an engine allowed to say otherwise
            // would manufacture precedent out of sequence.
            Self::FollowsFrom
            | Self::SatisfiesConstraint
            | Self::ViolatesConstraint
            | Self::Supersedes
            | Self::Contradicts => ReasonAsserter::AnySeat,
        }
    }

    /// Whether this kind says **how** something was done.
    #[must_use]
    pub const fn is_method(self) -> bool {
        matches!(self, Self::AchievedBy)
    }
}
