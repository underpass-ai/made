use made_proto::v1::{
    ArtifactRecord, GetArtifactRequest, ListArtifactsRequest, ListArtifactsResponse,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn get_artifact(
        &self,
        artifact_id: impl Into<String>,
    ) -> Result<ArtifactRecord, MadeClientError> {
        let response = self
            .rpc()
            .get_artifact(self.request(
                "/underpass.made.v1.MadeService/GetArtifact",
                GetArtifactRequest {
                    artifact_id: artifact_id.into(),
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.record.ok_or_else(|| {
            MadeClientError::ProtocolViolation("get artifact response has no record".to_owned())
        })
    }

    pub async fn list_artifacts(
        &self,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ListArtifactsResponse, MadeClientError> {
        self.rpc()
            .list_artifacts(self.request(
                "/underpass.made.v1.MadeService/ListArtifacts",
                ListArtifactsRequest { cursor, limit },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
}
