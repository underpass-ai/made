use serde::{Deserialize, Serialize};

use super::LoopRoundLimit;

/// How long an integrator's loop may go round before it stops itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_rounds: Option<LoopRoundLimit>,
    no_progress_rounds: LoopRoundLimit,
}

impl LoopLimits {
    #[must_use]
    pub const fn new(
        max_rounds: Option<LoopRoundLimit>,
        no_progress_rounds: LoopRoundLimit,
    ) -> Self {
        Self {
            max_rounds,
            no_progress_rounds,
        }
    }

    /// A ceiling on rounds, when the caller set one.
    #[must_use]
    pub const fn max_rounds(self) -> Option<LoopRoundLimit> {
        self.max_rounds
    }

    /// How many rounds with nothing new before the loop declares itself stuck.
    #[must_use]
    pub const fn no_progress_rounds(self) -> LoopRoundLimit {
        self.no_progress_rounds
    }
}

impl Default for LoopLimits {
    fn default() -> Self {
        Self {
            max_rounds: None,
            no_progress_rounds: LoopRoundLimit::default_without_progress(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_unbounded_rounds_and_three_without_progress() {
        let limits = LoopLimits::default();
        assert!(limits.max_rounds().is_none());
        assert_eq!(limits.no_progress_rounds().value(), 3);
    }
}
