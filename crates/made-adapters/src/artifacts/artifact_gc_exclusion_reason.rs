use serde::{Deserialize, Serialize};

/// Store-observed reason a blob was excluded from a GC preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactGcExclusionReason {
    LiveReference,
    RetentionWindow,
    ActiveUpload,
    ProtectedReference,
    NoReclaimableBytes,
}
