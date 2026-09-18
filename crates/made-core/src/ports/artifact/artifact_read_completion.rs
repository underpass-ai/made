use serde::{Deserialize, Serialize};

/// Whether a bounded read reached the end of the artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReadCompletion {
    More,
    Complete,
}

impl ArtifactReadCompletion {
    #[must_use]
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }
}
