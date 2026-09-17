use serde::{Deserialize, Serialize};

/// Whether a ceremony stage sees contributions from earlier stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorContext {
    Hidden,
    Visible,
}

impl PriorContext {
    #[must_use]
    pub const fn from_visible(visible: bool) -> Self {
        if visible {
            Self::Visible
        } else {
            Self::Hidden
        }
    }

    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}
