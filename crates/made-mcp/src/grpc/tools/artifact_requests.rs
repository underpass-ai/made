use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};

use super::super::json_to_proto as j2p;

pub(super) fn build_begin_artifact_upload_request(
    args: &Value,
) -> Result<pb::BeginArtifactUploadRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::BeginArtifactUploadRequest {
        requested_artifact_id: j2p::optional_str(obj, "requested_artifact_id")
            .map(ToOwned::to_owned),
        expected_digest: j2p::require_str(obj, "expected_digest")?.to_owned(),
        size_bytes: required_u64(obj, "size_bytes")?,
        media_type: j2p::require_str(obj, "media_type")?.to_owned(),
        provenance: Some(provenance(obj.get("provenance"))?),
        idempotency_key: j2p::require_str(obj, "idempotency_key")?.to_owned(),
    })
}

pub(super) fn build_put_artifact_chunk_request(
    args: &Value,
) -> Result<pb::PutArtifactChunkRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let encoded = j2p::require_str(obj, "bytes_base64")?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| format!("`bytes_base64` is not valid base64: {error}"))?;
    Ok(pb::PutArtifactChunkRequest {
        upload_id: j2p::require_str(obj, "upload_id")?.to_owned(),
        offset: required_u64(obj, "offset")?,
        bytes,
        chunk_digest: j2p::require_str(obj, "chunk_digest")?.to_owned(),
    })
}

pub(super) fn build_commit_artifact_upload_request(
    args: &Value,
) -> Result<pb::CommitArtifactUploadRequest, String> {
    Ok(pb::CommitArtifactUploadRequest {
        upload_id: upload_id(args)?,
    })
}

pub(super) fn build_abort_artifact_upload_request(
    args: &Value,
) -> Result<pb::AbortArtifactUploadRequest, String> {
    Ok(pb::AbortArtifactUploadRequest {
        upload_id: upload_id(args)?,
    })
}

pub(super) fn build_get_artifact_request(args: &Value) -> Result<pb::GetArtifactRequest, String> {
    Ok(pb::GetArtifactRequest {
        artifact_id: artifact_id(args)?,
    })
}

pub(super) fn build_list_artifacts_request(
    args: &Value,
) -> Result<pb::ListArtifactsRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ListArtifactsRequest {
        cursor: j2p::optional_str(obj, "cursor").map(ToOwned::to_owned),
        limit: optional_u32(obj, "limit")?,
    })
}

pub(super) fn build_read_artifact_chunk_request(
    args: &Value,
) -> Result<pb::ReadArtifactChunkRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ReadArtifactChunkRequest {
        artifact_id: j2p::require_str(obj, "artifact_id")?.to_owned(),
        offset: required_u64(obj, "offset")?,
        max_bytes: optional_u32(obj, "max_bytes")?,
    })
}

pub(super) fn build_tombstone_artifact_request(
    args: &Value,
) -> Result<pb::TombstoneArtifactRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::TombstoneArtifactRequest {
        artifact_id: j2p::require_str(obj, "artifact_id")?.to_owned(),
        actor: j2p::require_str(obj, "actor")?.to_owned(),
        policy: j2p::require_str(obj, "policy")?.to_owned(),
        retired_at: Some(j2p::parse_rfc3339_to_timestamp(j2p::require_str(
            obj,
            "retired_at",
        )?)?),
    })
}

fn upload_id(args: &Value) -> Result<String, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(j2p::require_str(obj, "upload_id")?.to_owned())
}

fn artifact_id(args: &Value) -> Result<String, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(j2p::require_str(obj, "artifact_id")?.to_owned())
}

fn provenance(value: Option<&Value>) -> Result<pb::ArtifactProvenance, String> {
    let value = value.ok_or_else(|| "missing required object `provenance`".to_owned())?;
    let obj = j2p::require_object(value, "provenance")?;
    let source_kind = match j2p::require_str(obj, "source_kind")? {
        "external_execution" => pb::ArtifactSourceKind::ExternalExecution,
        "fixture" => pb::ArtifactSourceKind::Fixture,
        "no_op" => pb::ArtifactSourceKind::NoOp,
        "generated_report" => pb::ArtifactSourceKind::GeneratedReport,
        "imported" => pb::ArtifactSourceKind::Imported,
        other => return Err(format!("unsupported artifact source kind `{other}`")),
    };
    Ok(pb::ArtifactProvenance {
        source_kind: source_kind as i32,
        execution_receipt_id: j2p::optional_str(obj, "execution_receipt_id").map(ToOwned::to_owned),
        operation_id: j2p::optional_str(obj, "operation_id").map(ToOwned::to_owned),
        accepted_claim_fence: j2p::optional_str(obj, "accepted_claim_fence").map(ToOwned::to_owned),
        observed_at: Some(j2p::parse_rfc3339_to_timestamp(j2p::require_str(
            obj,
            "observed_at",
        )?)?),
        import_ref: j2p::optional_str(obj, "import_ref").map(ToOwned::to_owned),
    })
}

fn required_u64(obj: &Map<String, Value>, key: &str) -> Result<u64, String> {
    obj.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing required unsigned integer `{key}`"))
}

fn optional_u32(obj: &Map<String, Value>, key: &str) -> Result<u32, String> {
    let Some(value) = obj.get(key) else {
        return Ok(0);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("`{key}` must be an unsigned integer"))?;
    u32::try_from(value).map_err(|_| format!("`{key}` is outside the uint32 range"))
}
