use std::fmt;

use serde::{Deserialize, Serialize};

use super::LoopRounds;

/// How many rounds of one looping composition a run has already done.
///
/// Counted on the link rather than derived from the instances, so that
/// advancing a run twice for the same round is refused by comparison
/// instead of opening a second instance nobody asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoopRound(u32);

impl LoopRound {
    /// Before the first round has run.
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn of(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Whether another round is still within the declared bound.
    #[must_use]
    pub const fn is_exhausted(self, bound: LoopRounds) -> bool {
        self.0 >= bound.get()
    }
}

impl fmt::Display for LoopRound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bounded_loop_stops_at_its_bound() {
        let bound = LoopRounds::new(2).unwrap();

        assert!(!LoopRound::ZERO.is_exhausted(bound));
        assert!(!LoopRound::ZERO.next().is_exhausted(bound));
        assert!(LoopRound::ZERO.next().next().is_exhausted(bound));
    }
}
