use serde::{Deserialize, Serialize};

use crate::value_objects::GlobalPosition;

/// Where one integrator's loop had got to the last time it asked.
///
/// Three numbers and nothing else: how far the feed had been projected
/// for this binding, how many of its deliveries had been closed, and
/// how many rounds it has spent since either of those last moved. A
/// round is one ask, which is the only thing that makes "going round
/// without getting anywhere" measurable — the ledger can say what is
/// outstanding but not how many times a host has come back to look at
/// it, and a lease is about exclusion rather than about rounds.
///
/// Kept on the binding because it is the binding's, and durable
/// because a process that restarted mid-loop would otherwise come back
/// with a fresh count and go round for ever.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopProgressMark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<GlobalPosition>,
    #[serde(default)]
    closed: u32,
    #[serde(default)]
    rounds: u32,
    #[serde(default)]
    stuck: u32,
}

impl LoopProgressMark {
    #[must_use]
    pub const fn new(head: Option<GlobalPosition>, closed: u32, rounds: u32, stuck: u32) -> Self {
        Self {
            head,
            closed,
            rounds,
            stuck,
        }
    }

    /// The mark after one more ask, given what that ask could see.
    ///
    /// Either the feed moved for this binding or something it was
    /// holding got closed; anything else is a round that changed
    /// nothing, and those are the ones counted consecutively. Both
    /// readings are of durable facts — the binding's own cursor and
    /// its ledger — so the same ask reaches the same conclusion before
    /// and after a restart.
    #[must_use]
    pub fn observing(self, head: Option<GlobalPosition>, closed: u32) -> Self {
        let moved = head != self.head || closed != self.closed;
        Self {
            head,
            closed,
            rounds: self.rounds.saturating_add(1),
            stuck: if moved {
                0
            } else {
                self.stuck.saturating_add(1)
            },
        }
    }

    /// How far the feed had been projected when this binding last asked.
    #[must_use]
    pub const fn head(self) -> Option<GlobalPosition> {
        self.head
    }

    /// Deliveries of this binding's that had been closed by then.
    #[must_use]
    pub const fn closed(self) -> u32 {
        self.closed
    }

    /// Asks this binding has made, over its life.
    #[must_use]
    pub const fn rounds(self) -> u32 {
        self.rounds
    }

    /// Consecutive asks that found the same head and the same closed
    /// count as the one before.
    #[must_use]
    pub const fn stuck(self) -> u32 {
        self.stuck
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: u64) -> GlobalPosition {
        GlobalPosition::new(value).unwrap()
    }

    #[test]
    fn a_round_that_saw_nothing_new_is_a_round_without_progress() {
        let first = LoopProgressMark::default().observing(Some(at(4)), 0);
        let second = first.observing(Some(at(4)), 0);
        let third = second.observing(Some(at(4)), 0);

        assert_eq!(third.rounds(), 3);
        assert_eq!(third.stuck(), 2, "the first ask at a head is not yet stuck");
    }

    #[test]
    fn the_feed_moving_is_progress() {
        let stuck = LoopProgressMark::default()
            .observing(Some(at(4)), 0)
            .observing(Some(at(4)), 0);
        assert_eq!(stuck.stuck(), 1);

        let moved = stuck.observing(Some(at(9)), 0);
        assert_eq!(moved.stuck(), 0);
        assert_eq!(moved.rounds(), 3, "rounds only ever go up");
        assert_eq!(moved.head(), Some(at(9)));
    }

    #[test]
    fn closing_a_delivery_is_progress_even_at_the_same_head() {
        let stuck = LoopProgressMark::default()
            .observing(Some(at(4)), 0)
            .observing(Some(at(4)), 0);

        let worked = stuck.observing(Some(at(4)), 1);
        assert_eq!(worked.stuck(), 0);
        assert_eq!(worked.closed(), 1);
    }
}
