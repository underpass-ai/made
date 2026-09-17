use serde::{Deserialize, Serialize};

/// Whether a recollection is all of what the scope held, or as much of
/// it as the budget allowed.
///
/// Said out loud rather than left to be inferred from a count nobody
/// has. A reader weighing what earlier sessions decided has to know
/// whether it is looking at the whole record or at the front of it; a
/// truncated recollection that did not say so would read as a complete
/// one, which is the only way a bound can turn into a lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecollectionCompleteness {
    /// Everything the scope held reached the reader.
    Whole,
    /// The budget stopped the rendering before the scope ran out.
    Truncated,
}

impl RecollectionCompleteness {
    #[must_use]
    pub const fn is_truncated(self) -> bool {
        matches!(self, Self::Truncated)
    }

    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Truncated => "truncated",
        }
    }
}
