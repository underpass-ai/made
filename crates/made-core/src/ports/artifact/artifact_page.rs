use crate::value_objects::ArtifactId;

use super::ArtifactRecord;

/// Stable artifact-id ordered metadata page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPage {
    pub items: Vec<ArtifactRecord>,
    pub next_after: Option<ArtifactId>,
}
