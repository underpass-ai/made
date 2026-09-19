use made_core::ports::ArtifactRecord;

use super::ArtifactCursor;

/// One bounded page of artifact metadata at the public application boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactListing {
    pub items: Vec<ArtifactRecord>,
    pub next_cursor: Option<ArtifactCursor>,
}
