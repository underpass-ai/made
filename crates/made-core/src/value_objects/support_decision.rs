use serde::{Deserialize, Serialize};

/// Semantic decision made about a claim and its cited evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportDecision {
    Supported,
    Unsupported,
}

impl SupportDecision {
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

impl From<bool> for SupportDecision {
    fn from(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }
}
