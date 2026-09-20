use std::fmt;

use serde::{Deserialize, Serialize};

/// What a full attention queue does with the next item.
///
/// One variant, named rather than implied: a queue that silently drops
/// is a queue nobody can reason about, and the day a second policy
/// exists this is the field that already carries the choice.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AttentionOverflowPolicy {
    /// Discard the oldest item nobody is waiting on, and record it.
    #[default]
    DropOldestNonBlocking,
}

impl AttentionOverflowPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DropOldestNonBlocking => "drop_oldest_non_blocking",
        }
    }
}

impl fmt::Display for AttentionOverflowPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
