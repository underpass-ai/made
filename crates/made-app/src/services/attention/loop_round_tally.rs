//! How many times one binding has been round, and how many of those
//! got nowhere.

use made_core::value_objects::LoopProgressMark;
use serde::Serialize;

/// The two counts a stall is decided from.
///
/// Read off the binding's own durable mark rather than inferred from
/// the ledger. The ledger can say what is outstanding; it cannot say
/// how many times a host came back to look at it, because a host that
/// holds a live lease is handed nothing and leaves no trace of having
/// asked. Rounds are asks, so asks are what is counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LoopRoundTally {
    rounds: u32,
    stuck: u32,
}

impl LoopRoundTally {
    #[must_use]
    pub const fn new(rounds: u32, stuck: u32) -> Self {
        Self { rounds, stuck }
    }

    /// The tally this binding's mark stands for.
    #[must_use]
    pub const fn of(mark: LoopProgressMark) -> Self {
        Self {
            rounds: mark.rounds(),
            stuck: mark.stuck(),
        }
    }

    /// Asks this binding has made, over its life.
    #[must_use]
    pub const fn rounds(self) -> u32 {
        self.rounds
    }

    /// Consecutive asks that found the feed and the ledger where they
    /// left them.
    #[must_use]
    pub const fn stuck(self) -> u32 {
        self.stuck
    }
}
