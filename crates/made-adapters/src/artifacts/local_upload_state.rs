use made_core::value_objects::ArtifactRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "artifact")]
pub(super) enum LocalUploadState {
    Active,
    Committed(ArtifactRef),
    Aborted,
}
