use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::{json, Value};
use tonic::transport::Channel;

use crate::protocol::{
    ToolError, ABORT_ARTIFACT_UPLOAD_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL, COMMIT_ARTIFACT_UPLOAD_TOOL,
    GET_ARTIFACT_TOOL, LIST_ARTIFACTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL, READ_ARTIFACT_CHUNK_TOOL,
    TOMBSTONE_ARTIFACT_TOOL,
};

use super::artifact_requests as request;
use super::{bad_request, p2j};

const TOOLS: [&str; 8] = [
    BEGIN_ARTIFACT_UPLOAD_TOOL,
    PUT_ARTIFACT_CHUNK_TOOL,
    COMMIT_ARTIFACT_UPLOAD_TOOL,
    ABORT_ARTIFACT_UPLOAD_TOOL,
    GET_ARTIFACT_TOOL,
    LIST_ARTIFACTS_TOOL,
    READ_ARTIFACT_CHUNK_TOOL,
    TOMBSTONE_ARTIFACT_TOOL,
];

pub(super) fn handles(name: &str) -> bool {
    TOOLS.contains(&name)
}

pub(super) async fn dispatch(
    client: &mut MadeServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, impl tonic::service::Interceptor>,
    >,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_begin_artifact_upload" => {
            let response = client
                .begin_artifact_upload(
                    request::build_begin_artifact_upload_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            p2j::artifact_upload_status_to_json(response.upload)
        }
        "made_put_artifact_chunk" => {
            let response = client
                .put_artifact_chunk(
                    request::build_put_artifact_chunk_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            p2j::artifact_upload_status_to_json(response.upload)
        }
        "made_commit_artifact_upload" => {
            let response = client
                .commit_artifact_upload(
                    request::build_commit_artifact_upload_request(arguments)
                        .map_err(bad_request)?,
                )
                .await?
                .into_inner();
            p2j::artifact_ref_to_json(response.artifact.as_ref())
        }
        "made_abort_artifact_upload" => {
            client
                .abort_artifact_upload(
                    request::build_abort_artifact_upload_request(arguments).map_err(bad_request)?,
                )
                .await?;
            Ok(json!({ "aborted": true }))
        }
        "made_get_artifact" => {
            let response = client
                .get_artifact(request::build_get_artifact_request(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            p2j::artifact_record_to_json(response.record.as_ref())
        }
        "made_list_artifacts" => {
            let response = client
                .list_artifacts(
                    request::build_list_artifacts_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(p2j::artifact_listing_to_json(&response))
        }
        "made_read_artifact_chunk" => {
            let response = client
                .read_artifact_chunk(
                    request::build_read_artifact_chunk_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            Ok(p2j::artifact_chunk_to_json(response))
        }
        "made_tombstone_artifact" => {
            let response = client
                .tombstone_artifact(
                    request::build_tombstone_artifact_request(arguments).map_err(bad_request)?,
                )
                .await?
                .into_inner();
            p2j::artifact_tombstone_to_json(response.tombstone.as_ref())
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown artifact tool `{other}`"
        ))),
    }
}
