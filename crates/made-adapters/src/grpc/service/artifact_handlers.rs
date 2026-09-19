use made_app::artifacts::ArtifactCursor;
use made_core::ports::ArtifactUploadId;
use made_core::value_objects::ArtifactId;

use super::{
    artifact_chunk_to_proto, artifact_error_to_status, artifact_page_limit_from_proto,
    artifact_record_to_proto, artifact_ref_to_proto, artifact_tombstone_to_proto,
    artifact_upload_status_to_proto, begin_artifact_upload_from_proto, domain_error_to_status,
    link_span_to_metadata, pb, put_artifact_chunk_from_proto, read_artifact_chunk_from_proto,
    tombstone_artifact_from_proto, GrpcResult, MadeGrpcService, Request, Response, Status,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.begin_artifact_upload", skip_all)]
    pub(super) async fn handle_begin_artifact_upload(
        &self,
        request: Request<pb::BeginArtifactUploadRequest>,
    ) -> GrpcResult<pb::BeginArtifactUploadResponse> {
        link_span_to_metadata(&request);
        let request = begin_artifact_upload_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let upload = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .begin_upload(request)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::BeginArtifactUploadResponse {
            upload: Some(artifact_upload_status_to_proto(&upload)),
        }))
    }

    #[tracing::instrument(name = "rpc.put_artifact_chunk", skip_all)]
    pub(super) async fn handle_put_artifact_chunk(
        &self,
        request: Request<pb::PutArtifactChunkRequest>,
    ) -> GrpcResult<pb::PutArtifactChunkResponse> {
        link_span_to_metadata(&request);
        let request =
            put_artifact_chunk_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        let upload = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .put_chunk(request)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::PutArtifactChunkResponse {
            upload: Some(artifact_upload_status_to_proto(&upload)),
        }))
    }

    #[tracing::instrument(name = "rpc.commit_artifact_upload", skip_all)]
    pub(super) async fn handle_commit_artifact_upload(
        &self,
        request: Request<pb::CommitArtifactUploadRequest>,
    ) -> GrpcResult<pb::CommitArtifactUploadResponse> {
        link_span_to_metadata(&request);
        let upload_id = ArtifactUploadId::new(request.into_inner().upload_id)
            .map_err(domain_error_to_status)?;
        let artifact = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .commit_upload(&upload_id)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::CommitArtifactUploadResponse {
            artifact: Some(artifact_ref_to_proto(&artifact)),
        }))
    }

    #[tracing::instrument(name = "rpc.abort_artifact_upload", skip_all)]
    pub(super) async fn handle_abort_artifact_upload(
        &self,
        request: Request<pb::AbortArtifactUploadRequest>,
    ) -> GrpcResult<pb::AbortArtifactUploadResponse> {
        link_span_to_metadata(&request);
        let upload_id = ArtifactUploadId::new(request.into_inner().upload_id)
            .map_err(domain_error_to_status)?;
        self.artifact_service()
            .ok_or_else(artifact_unavailable)?
            .abort_upload(&upload_id)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::AbortArtifactUploadResponse {}))
    }

    #[tracing::instrument(name = "rpc.get_artifact", skip_all)]
    pub(super) async fn handle_get_artifact(
        &self,
        request: Request<pb::GetArtifactRequest>,
    ) -> GrpcResult<pb::GetArtifactResponse> {
        link_span_to_metadata(&request);
        let artifact_id =
            ArtifactId::new(request.into_inner().artifact_id).map_err(domain_error_to_status)?;
        let record = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .get(&artifact_id)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::GetArtifactResponse {
            record: Some(artifact_record_to_proto(&record).map_err(domain_error_to_status)?),
        }))
    }

    #[tracing::instrument(name = "rpc.list_artifacts", skip_all)]
    pub(super) async fn handle_list_artifacts(
        &self,
        request: Request<pb::ListArtifactsRequest>,
    ) -> GrpcResult<pb::ListArtifactsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let cursor = request
            .cursor
            .map(ArtifactCursor::parse)
            .transpose()
            .map_err(artifact_error_to_status)?;
        let limit =
            artifact_page_limit_from_proto(request.limit).map_err(domain_error_to_status)?;
        let listing = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .list_page(cursor.as_ref(), limit)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::ListArtifactsResponse {
            artifacts: listing
                .items
                .iter()
                .map(artifact_record_to_proto)
                .collect::<Result<_, _>>()
                .map_err(domain_error_to_status)?,
            next_cursor: listing.next_cursor.map(|cursor| cursor.as_str().to_owned()),
        }))
    }

    #[tracing::instrument(name = "rpc.read_artifact_chunk", skip_all)]
    pub(super) async fn handle_read_artifact_chunk(
        &self,
        request: Request<pb::ReadArtifactChunkRequest>,
    ) -> GrpcResult<pb::ReadArtifactChunkResponse> {
        link_span_to_metadata(&request);
        let request =
            read_artifact_chunk_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        let chunk = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .read_chunk(request)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(artifact_chunk_to_proto(chunk)))
    }

    #[tracing::instrument(name = "rpc.tombstone_artifact", skip_all)]
    pub(super) async fn handle_tombstone_artifact(
        &self,
        request: Request<pb::TombstoneArtifactRequest>,
    ) -> GrpcResult<pb::TombstoneArtifactResponse> {
        link_span_to_metadata(&request);
        let command =
            tombstone_artifact_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        let tombstone = self
            .artifact_service()
            .ok_or_else(artifact_unavailable)?
            .tombstone(command)
            .await
            .map_err(artifact_error_to_status)?;
        Ok(Response::new(pb::TombstoneArtifactResponse {
            tombstone: Some(
                artifact_tombstone_to_proto(&tombstone).map_err(domain_error_to_status)?,
            ),
        }))
    }
}

fn artifact_unavailable() -> Status {
    Status::unavailable("artifact storage is not configured")
}
