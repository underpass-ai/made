//! [`NoProgressDetector`] — telling a loop that is working slowly from
//! one that has stopped working.
//!
//! An integrator's loop is a host asking the same question over and
//! over. Nothing in that shape says when to give up, and a loop with
//! no answer to that question either stops too early — on the first
//! quiet round — or never, which is the expensive one.
//!
//! The reading is pure and the evidence is durable. Nothing here holds
//! a counter: the ledger already records how many times each item was
//! handed over and when one was acted on, so the same answer comes out
//! after a restart as before it.

use made_core::value_objects::LoopLimits;

use super::{LoopRoundTally, LoopStall};

/// Reads a binding's rounds against the limits its policy set.
#[derive(Debug, Clone, Copy)]
pub struct NoProgressDetector {
    limits: LoopLimits,
}

impl NoProgressDetector {
    #[must_use]
    pub const fn new(limits: LoopLimits) -> Self {
        Self { limits }
    }

    /// Whether this loop should stop, and which of the two reasons.
    ///
    /// The ceiling is read before the stall, because a loop that has
    /// used up its rounds is finished whether or not it was getting
    /// anywhere, and telling a host it is merely stuck would invite it
    /// to try once more.
    #[must_use]
    pub fn detect(&self, rounds: LoopRoundTally) -> Option<LoopStall> {
        if self
            .limits
            .max_rounds()
            .is_some_and(|limit| rounds.handed_over() >= limit.value())
        {
            return Some(LoopStall::RoundLimit);
        }
        (rounds.stuck() >= self.limits.no_progress_rounds().value())
            .then_some(LoopStall::NoProgress)
    }
}

#[cfg(test)]
mod tests;
