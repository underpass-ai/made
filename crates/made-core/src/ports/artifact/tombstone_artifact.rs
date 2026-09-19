use time::OffsetDateTime;

use crate::value_objects::ArtifactId;

use super::{ArtifactRetentionActor, ArtifactRetentionPolicy};

/// Host-authorized retention command. Authorization remains outside C5.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TombstoneArtifact {
    pub artifact_id: ArtifactId,
    pub actor: ArtifactRetentionActor,
    pub policy: ArtifactRetentionPolicy,
    pub retired_at: OffsetDateTime,
}
