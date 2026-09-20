use std::fmt;

use serde::{Deserialize, Serialize};

/// Whether pending work follows a destination that was replaced.
///
/// Named rather than a bare flag because the two answers are different
/// promises: one says the question stays with the seat, the other says
/// it stays with the process that was asked and dies with it.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum FollowReplacement {
    /// Leave the delivery superseded and visible; do not re-address it.
    #[default]
    Stay,
    /// Re-address pending deliveries to the replacement.
    Follow,
}

impl FollowReplacement {
    #[must_use]
    pub const fn follows(self) -> bool {
        matches!(self, Self::Follow)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stay => "stay",
            Self::Follow => "follow",
        }
    }
}

impl fmt::Display for FollowReplacement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
