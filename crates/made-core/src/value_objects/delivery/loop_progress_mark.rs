use serde::{Deserialize, Serialize};

use crate::value_objects::GlobalPosition;

use super::Owed;

/// Where one integrator's loop had got to the last time it asked.
///
/// Three numbers and nothing else: the furthest record this binding
/// has been offered something from, how many of its deliveries had
/// been closed, and how many asks it has spent since either of those
/// last moved. A round is one ask, which is the only thing that makes
/// "going round without getting anywhere" measurable — the ledger can
/// say what is outstanding but not how many times a host has come back
/// to look at it, and a lease is about exclusion rather than rounds.
///
/// The head is this binding's own, not the engine's. A cursor walks
/// the global feed and advances over every ceremony in the
/// deployment, so a busy neighbour would move it every second and hide
/// a stall for ever.
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
    /// A round counts against the loop only when the binding was
    /// holding work and neither the news nor the work moved. The two
    /// halves matter separately. Without the first, a loop that polls
    /// while an agent works a long step is declared stuck on its third
    /// second. Without the second, a host that takes something and
    /// does nothing with it is never noticed.
    ///
    /// Both readings are of durable facts — what this binding has been
    /// offered, and what it has closed — so the same ask reaches the
    /// same conclusion before and after a restart.
    #[must_use]
    pub fn observing(self, head: Option<GlobalPosition>, closed: u32, owed: Owed) -> Self {
        let moved = head != self.head || closed != self.closed;
        let idle = owed == Owed::Nothing;
        Self {
            head,
            closed,
            rounds: self.rounds.saturating_add(1),
            stuck: if moved || idle {
                0
            } else {
                self.stuck.saturating_add(1)
            },
        }
    }

    /// Whether this mark says anything the last one did not.
    ///
    /// The ask count always moves, and on its own it changes no
    /// decision unless the policy set a ceiling. Everything else is
    /// what the stall is read from.
    #[must_use]
    pub fn differs_from(self, previous: Self) -> bool {
        self.head != previous.head || self.closed != previous.closed || self.stuck != previous.stuck
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
        let first = LoopProgressMark::default().observing(Some(at(4)), 0, Owed::Something);
        let second = first.observing(Some(at(4)), 0, Owed::Something);
        let third = second.observing(Some(at(4)), 0, Owed::Something);

        assert_eq!(third.rounds(), 3);
        assert_eq!(third.stuck(), 2, "the first ask at a head is not yet stuck");
    }

    #[test]
    fn the_feed_moving_is_progress() {
        let stuck = LoopProgressMark::default()
            .observing(Some(at(4)), 0, Owed::Something)
            .observing(Some(at(4)), 0, Owed::Something);
        assert_eq!(stuck.stuck(), 1);

        let moved = stuck.observing(Some(at(9)), 0, Owed::Something);
        assert_eq!(moved.stuck(), 0);
        assert_eq!(moved.rounds(), 3, "rounds only ever go up");
        assert_eq!(moved.head(), Some(at(9)));
    }

    /// A loop with an empty queue is not stuck, however long it waits.
    ///
    /// Waiting is what a loop does while an agent works a long step or
    /// a person thinks. Counting those asks would declare every
    /// healthy idle loop blocked on its third poll, and a poll is a
    /// second by default.
    #[test]
    fn a_quiet_ask_with_nothing_owed_is_not_a_round_without_progress() {
        let mut mark = LoopProgressMark::default();
        for _ in 0..10 {
            mark = mark.observing(Some(at(4)), 0, Owed::Nothing);
        }

        assert_eq!(mark.stuck(), 0, "an idle loop is waiting, not stuck");
        assert_eq!(mark.rounds(), 10, "the asks still counted");
    }

    /// And a loop that was handed something and did nothing with it is.
    #[test]
    fn a_quiet_ask_while_holding_work_is_a_round_without_progress() {
        let mark = LoopProgressMark::default()
            .observing(Some(at(4)), 0, Owed::Something)
            .observing(Some(at(4)), 0, Owed::Something)
            .observing(Some(at(4)), 0, Owed::Something);

        assert_eq!(mark.stuck(), 2);
    }

    /// Finishing the work clears the count even at an unmoved head.
    #[test]
    fn an_emptied_queue_clears_the_count() {
        let stuck = LoopProgressMark::default()
            .observing(Some(at(4)), 0, Owed::Something)
            .observing(Some(at(4)), 0, Owed::Something);
        assert_eq!(stuck.stuck(), 1);

        assert_eq!(stuck.observing(Some(at(4)), 1, Owed::Nothing).stuck(), 0);
    }

    #[test]
    fn a_mark_that_says_nothing_new_is_not_worth_writing_down() {
        let first = LoopProgressMark::default().observing(Some(at(4)), 0, Owed::Nothing);
        let second = first.observing(Some(at(4)), 0, Owed::Nothing);

        assert!(!second.differs_from(first), "only the ask count moved");
        assert!(second
            .observing(Some(at(9)), 0, Owed::Nothing)
            .differs_from(second));
    }

    #[test]
    fn closing_a_delivery_is_progress_even_at_the_same_head() {
        let stuck = LoopProgressMark::default()
            .observing(Some(at(4)), 0, Owed::Something)
            .observing(Some(at(4)), 0, Owed::Something);

        let worked = stuck.observing(Some(at(4)), 1, Owed::Something);
        assert_eq!(worked.stuck(), 0);
        assert_eq!(worked.closed(), 1);
    }
}
