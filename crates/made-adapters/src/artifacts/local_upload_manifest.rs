use made_core::ports::{ArtifactUploadId, BeginArtifactUpload};
use serde::{Deserialize, Serialize};

use super::local_upload_state::LocalUploadState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LocalUploadManifest {
    pub(super) upload_id: ArtifactUploadId,
    pub(super) request: BeginArtifactUpload,
    pub(super) state: LocalUploadState,
}
