use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{ArtifactDigest, AuthorizationEvidence};

use super::{ArtifactRetentionActor, ArtifactRetentionPolicy};

/// Durable audit record left when artifact content is retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTombstone {
    pub actor: ArtifactRetentionActor,
    pub policy: ArtifactRetentionPolicy,
    #[serde(with = "time::serde::rfc3339")]
    pub retired_at: OffsetDateTime,
    pub digest: ArtifactDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<AuthorizationEvidence>,
}

impl ArtifactTombstone {
    /// Whether both records describe the same retirement operation.
    ///
    /// Authorization is deliberately excluded: an exact business retry keeps the
    /// evidence sealed by the first successful append instead of conflicting with
    /// a later authorization decision.
    #[must_use]
    pub fn same_retirement_as(&self, other: &Self) -> bool {
        self.actor == other.actor
            && self.policy == other.policy
            && self.retired_at == other.retired_at
            && self.digest == other.digest
    }
}
