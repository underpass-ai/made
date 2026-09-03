use serde::{Deserialize, Serialize};

/// Whether an agent should seek an intentionally different proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DiversityPreference {
    #[default]
    Standard,
    Diverse,
}

impl DiversityPreference {
    #[must_use]
    pub const fn is_diverse(self) -> bool {
        matches!(self, Self::Diverse)
    }
}

impl From<bool> for DiversityPreference {
    fn from(diverse: bool) -> Self {
        if diverse {
            Self::Diverse
        } else {
            Self::Standard
        }
    }
}
