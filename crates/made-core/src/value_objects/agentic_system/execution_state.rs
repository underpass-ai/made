use std::fmt;

use serde::{Deserialize, Serialize};

use super::LinkStatus;

/// Where a whole system run has got to.
///
/// Derived from its links rather than set by hand, so the summary and
/// the detail cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Planned,
    Running,
    Completed,
    Failed,
}

impl ExecutionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Fold the state of every composed ceremony into the run's own.
    ///
    /// A run with a failed ceremony has failed even if others are
    /// still going: what it promised is no longer deliverable, and
    /// calling it "running" would invite somebody to wait for it.
    #[must_use]
    pub fn of(links: impl IntoIterator<Item = LinkStatus>) -> Self {
        let mut seen = false;
        let mut every_settled = true;
        let mut any_failed = false;
        let mut any_started = false;
        for status in links {
            seen = true;
            every_settled &= status.is_settled();
            any_failed |= matches!(status, LinkStatus::Failed);
            any_started |= !matches!(status, LinkStatus::Pending);
        }
        if any_failed {
            return Self::Failed;
        }
        if !seen || !any_started {
            return Self::Planned;
        }
        if every_settled {
            Self::Completed
        } else {
            Self::Running
        }
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_failure_fails_the_run_however_the_rest_are_going() {
        assert_eq!(
            ExecutionState::of([LinkStatus::Started, LinkStatus::Failed]),
            ExecutionState::Failed
        );
    }

    #[test]
    fn a_run_whose_ceremonies_were_all_skipped_is_finished_not_running() {
        assert_eq!(
            ExecutionState::of([LinkStatus::Skipped, LinkStatus::Completed]),
            ExecutionState::Completed
        );
    }

    #[test]
    fn nothing_started_yet_is_planned() {
        assert_eq!(
            ExecutionState::of([LinkStatus::Pending, LinkStatus::Pending]),
            ExecutionState::Planned
        );
        assert_eq!(ExecutionState::of([]), ExecutionState::Planned);
    }
}
