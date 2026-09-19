use made_app::artifacts::{ArtifactCursor, ArtifactListing, ArtifactService};
use made_core::ports::{
    ArtifactChunkPage, ArtifactPageLimit, ArtifactRecord, ArtifactStoreError, ArtifactTombstone,
    ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{ArtifactId, ArtifactRef, AuthorizationAction};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn begin_artifact_upload(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        match request.requested_artifact_id.as_ref() {
            Some(artifact_id) => self.require_authorized_artifact_action(
                AuthorizationAction::BeginArtifactUpload,
                artifact_id,
            )?,
            None => {
                self.require_authorized_global_action(AuthorizationAction::BeginArtifactUpload)?;
            }
        }
        self.artifact_service()?.begin_upload(request).await
    }

    pub async fn put_artifact_chunk(
        &self,
        request: PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.require_authorized_action(AuthorizationAction::PutArtifactChunk)?;
        let artifact_id = self
            .artifact_service()?
            .artifact_id_for_upload(&request.upload_id)
            .await?;
        self.require_authorized_artifact_action(
            AuthorizationAction::PutArtifactChunk,
            &artifact_id,
        )?;
        self.artifact_service()?.put_chunk(request).await
    }

    pub async fn commit_artifact_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.require_authorized_action(AuthorizationAction::CommitArtifactUpload)?;
        let artifact_id = self
            .artifact_service()?
            .artifact_id_for_upload(upload_id)
            .await?;
        self.require_authorized_artifact_action(
            AuthorizationAction::CommitArtifactUpload,
            &artifact_id,
        )?;
        self.artifact_service()?.commit_upload(upload_id).await
    }

    pub async fn abort_artifact_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<(), ArtifactStoreError> {
        self.require_authorized_action(AuthorizationAction::AbortArtifactUpload)?;
        let artifact_id = self
            .artifact_service()?
            .artifact_id_for_upload(upload_id)
            .await?;
        self.require_authorized_artifact_action(
            AuthorizationAction::AbortArtifactUpload,
            &artifact_id,
        )?;
        self.artifact_service()?.abort_upload(upload_id).await
    }

    pub async fn get_artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<ArtifactRecord, ArtifactStoreError> {
        self.require_authorized_artifact_action(AuthorizationAction::GetArtifact, artifact_id)?;
        self.artifact_service()?.get(artifact_id).await
    }

    pub async fn list_artifacts(
        &self,
        cursor: Option<&ArtifactCursor>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactListing, ArtifactStoreError> {
        self.require_authorized_global_action(AuthorizationAction::ListArtifacts)?;
        self.artifact_service()?.list_page(cursor, limit).await
    }

    pub async fn read_artifact_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.require_authorized_artifact_action(
            AuthorizationAction::ReadArtifactChunk,
            &request.artifact_id,
        )?;
        self.artifact_service()?.read_chunk(request).await
    }

    pub async fn tombstone_artifact(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.require_authorized_artifact_action(
            AuthorizationAction::TombstoneArtifact,
            &command.artifact_id,
        )?;
        self.artifact_service()?.tombstone(command).await
    }

    fn artifact_service(&self) -> Result<&ArtifactService, ArtifactStoreError> {
        self.artifacts
            .as_deref()
            .ok_or(ArtifactStoreError::StorageUnavailable)
    }
}
