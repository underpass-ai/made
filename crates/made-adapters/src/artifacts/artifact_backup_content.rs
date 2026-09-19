use serde::{Deserialize, Serialize};

/// Whether a backup entry includes bytes or only durable retired metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactBackupContent {
    #[default]
    Present,
    RetiredMetadataOnly,
}
