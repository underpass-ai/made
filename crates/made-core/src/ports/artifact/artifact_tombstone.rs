use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::ArtifactDigest;

use super::{ArtifactRetentionActor, ArtifactRetentionPolicy};

/// Durable audit record left when artifact content is retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTombstone {
    pub actor: ArtifactRetentionActor,
    pub policy: ArtifactRetentionPolicy,
    #[serde(with = "time::serde::rfc3339")]
    pub retired_at: OffsetDateTime,
    pub digest: ArtifactDigest,
}
