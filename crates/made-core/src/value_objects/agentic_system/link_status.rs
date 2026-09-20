use std::fmt;

use serde::{Deserialize, Serialize};

/// Where one composed ceremony has got to inside a run.
///
/// `Skipped` is not a failure and not a success: it is what a run says
/// when a participant the ceremony needed could not be materialized.
/// Recording it as anything else would hide the gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    Pending,
    Started,
    Completed,
    Failed,
    Skipped,
}

impl LinkStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    /// Whether a link in this state can still change.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Skipped)
    }

    /// Whether dependants of this link may start.
    ///
    /// Only completion releases them. A skipped ceremony produced no
    /// outputs, so anything that read them would be reading nothing.
    #[must_use]
    pub const fn releases_dependants(self) -> bool {
        matches!(self, Self::Completed)
    }

    /// The CSS class the diagram gives a node in this state.
    #[must_use]
    pub const fn diagram_class(self) -> &'static str {
        match self {
            Self::Pending => "linkPending",
            Self::Started => "linkStarted",
            Self::Completed => "linkCompleted",
            Self::Failed => "linkFailed",
            Self::Skipped => "linkSkipped",
        }
    }
}

impl fmt::Display for LinkStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skipped_ceremony_settles_without_releasing_what_depended_on_it() {
        assert!(LinkStatus::Skipped.is_settled());
        assert!(!LinkStatus::Skipped.releases_dependants());
        assert!(LinkStatus::Completed.releases_dependants());
        assert!(!LinkStatus::Pending.is_settled());
    }
}
