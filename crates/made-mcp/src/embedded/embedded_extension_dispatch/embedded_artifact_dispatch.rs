use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use made_app::artifacts::ArtifactCursor;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactRecord, ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactTombstone,
    ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactImportRef, ArtifactMediaType, ArtifactProvenance,
    ArtifactRef, ArtifactSizeBytes, ArtifactSourceKind, ExecutionOperationId, ExecutionReceiptId,
    StepClaimFence,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::protocol::{
    tool_success_result, ToolError, ABORT_ARTIFACT_UPLOAD_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL,
    COMMIT_ARTIFACT_UPLOAD_TOOL, GET_ARTIFACT_TOOL, LIST_ARTIFACTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL,
    READ_ARTIFACT_CHUNK_TOOL, TOMBSTONE_ARTIFACT_TOOL,
};

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
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let value = match name {
        "made_begin_artifact_upload" => {
            let status = made.begin_artifact_upload(begin(arguments)?).await?;
            upload_status(&status)
        }
        "made_put_artifact_chunk" => {
            let status = made.put_artifact_chunk(put(arguments)?).await?;
            upload_status(&status)
        }
        "made_commit_artifact_upload" => {
            let artifact = made.commit_artifact_upload(&upload_id(arguments)?).await?;
            artifact_ref(&artifact)
        }
        "made_abort_artifact_upload" => {
            made.abort_artifact_upload(&upload_id(arguments)?).await?;
            json!({ "aborted": true })
        }
        "made_get_artifact" => artifact_record(&made.get_artifact(&artifact_id(arguments)?).await?),
        "made_list_artifacts" => {
            let obj = object(arguments)?;
            let cursor = optional_string(obj, "cursor")
                .map(ArtifactCursor::parse)
                .transpose()?;
            let limit = match optional_u64(obj, "limit")? {
                None => ArtifactPageLimit::default(),
                Some(value) => ArtifactPageLimit::new(u16::try_from(value).map_err(|_| {
                    ToolError::invalid_request("`limit` is outside the uint16 range")
                })?)?,
            };
            let listing = made.list_artifacts(cursor.as_ref(), limit).await?;
            json!({
                "artifacts": listing.items.iter().map(artifact_record).collect::<Vec<_>>(),
                "next_cursor": listing.next_cursor.map(|cursor| cursor.as_str().to_owned()),
            })
        }
        "made_read_artifact_chunk" => {
            let obj = object(arguments)?;
            let max_bytes = match optional_u64(obj, "max_bytes")? {
                None => ArtifactChunkLimit::DEFAULT,
                Some(value) => ArtifactChunkLimit::new(u32::try_from(value).map_err(|_| {
                    ToolError::invalid_request("`max_bytes` is outside the uint32 range")
                })?)?,
            };
            let chunk = made
                .read_artifact_chunk(ReadArtifactChunk {
                    artifact_id: ArtifactId::new(required_string(obj, "artifact_id")?)?,
                    offset: ArtifactByteOffset::new(required_u64(obj, "offset")?),
                    max_bytes,
                })
                .await?;
            let eof = chunk.is_complete();
            json!({
                "bytes_base64": STANDARD.encode(chunk.bytes),
                "next_offset": chunk.next_offset.get(),
                "chunk_digest": chunk.chunk_digest.as_str(),
                "eof": eof,
            })
        }
        "made_tombstone_artifact" => {
            let obj = object(arguments)?;
            let tombstone = made
                .tombstone_artifact(TombstoneArtifact {
                    artifact_id: ArtifactId::new(required_string(obj, "artifact_id")?)?,
                    actor: ArtifactRetentionActor::new(required_string(obj, "actor")?)?,
                    policy: ArtifactRetentionPolicy::new(required_string(obj, "policy")?)?,
                    retired_at: parse_time(required_string(obj, "retired_at")?)?,
                })
                .await?;
            artifact_tombstone(&tombstone)
        }
        other => {
            return Err(ToolError::invalid_request(format!(
                "unknown artifact tool `{other}`"
            )))
        }
    };
    Ok(tool_success_result(value))
}

fn begin(arguments: &Value) -> Result<BeginArtifactUpload, ToolError> {
    let obj = object(arguments)?;
    Ok(BeginArtifactUpload {
        requested_artifact_id: optional_string(obj, "requested_artifact_id")
            .map(ArtifactId::new)
            .transpose()?,
        expected_digest: ArtifactDigest::new(required_string(obj, "expected_digest")?)?,
        size_bytes: ArtifactSizeBytes::new(required_u64(obj, "size_bytes")?),
        media_type: ArtifactMediaType::new(required_string(obj, "media_type")?)?,
        provenance: provenance(
            obj.get("provenance")
                .ok_or_else(|| ToolError::invalid_request("missing required `provenance`"))?,
        )?,
        idempotency_key: ArtifactIdempotencyKey::new(required_string(obj, "idempotency_key")?)?,
    })
}

fn put(arguments: &Value) -> Result<PutArtifactChunk, ToolError> {
    let obj = object(arguments)?;
    let bytes = STANDARD
        .decode(required_string(obj, "bytes_base64")?)
        .map_err(|error| ToolError::invalid_request(format!("invalid `bytes_base64`: {error}")))?;
    Ok(PutArtifactChunk {
        upload_id: ArtifactUploadId::new(required_string(obj, "upload_id")?)?,
        offset: ArtifactByteOffset::new(required_u64(obj, "offset")?),
        bytes,
        chunk_digest: ArtifactDigest::new(required_string(obj, "chunk_digest")?)?,
    })
}

fn provenance(value: &Value) -> Result<ArtifactProvenance, ToolError> {
    let obj = object(value)?;
    let observed_at = parse_time(required_string(obj, "observed_at")?)?;
    match required_string(obj, "source_kind")? {
        "external_execution" | "fixture" | "no_op" => {
            let source_kind = match required_string(obj, "source_kind")? {
                "external_execution" => ArtifactSourceKind::ExternalExecution,
                "fixture" => ArtifactSourceKind::Fixture,
                _ => ArtifactSourceKind::NoOp,
            };
            Ok(ArtifactProvenance::execution(
                source_kind,
                ExecutionReceiptId::new(required_string(obj, "execution_receipt_id")?)?,
                ExecutionOperationId::new(required_string(obj, "operation_id")?)?,
                StepClaimFence::new(required_string(obj, "accepted_claim_fence")?)?,
                observed_at,
            )?)
        }
        "generated_report" => Ok(ArtifactProvenance::generated_report(observed_at)),
        "imported" => Ok(ArtifactProvenance::imported(
            ArtifactImportRef::new(required_string(obj, "import_ref")?)?,
            observed_at,
        )),
        other => Err(ToolError::invalid_request(format!(
            "unsupported artifact source kind `{other}`"
        ))),
    }
}

fn upload_id(arguments: &Value) -> Result<ArtifactUploadId, ToolError> {
    ArtifactUploadId::new(required_string(object(arguments)?, "upload_id")?).map_err(Into::into)
}

fn artifact_id(arguments: &Value) -> Result<ArtifactId, ToolError> {
    ArtifactId::new(required_string(object(arguments)?, "artifact_id")?).map_err(Into::into)
}

fn upload_status(status: &ArtifactUploadStatus) -> Value {
    json!({
        "upload_id": status.upload_id.as_str(),
        "next_offset": status.next_offset.get(),
        "chunk_limit": status.chunk_limit.get(),
    })
}

pub(in crate::embedded) fn artifact_ref(artifact: &ArtifactRef) -> Value {
    json!({
        "artifact_id": artifact.artifact_id().as_str(),
        "digest": artifact.digest().as_str(),
        "size_bytes": artifact.size_bytes().get(),
        "media_type": artifact.media_type().as_str(),
        "provenance": artifact_provenance(artifact.provenance()),
    })
}

fn artifact_provenance(value: &ArtifactProvenance) -> Value {
    let source_kind = match value.source_kind() {
        ArtifactSourceKind::ExternalExecution => "external_execution",
        ArtifactSourceKind::Fixture => "fixture",
        ArtifactSourceKind::NoOp => "no_op",
        ArtifactSourceKind::GeneratedReport => "generated_report",
        ArtifactSourceKind::Imported => "imported",
    };
    json!({
        "source_kind": source_kind,
        "execution_receipt_id": value.execution_receipt_id().map(ToString::to_string),
        "operation_id": value.operation_id().map(ToString::to_string),
        "accepted_claim_fence": value.accepted_claim_fence().map(StepClaimFence::as_str),
        "observed_at": format_time(value.observed_at()),
        "import_ref": value.import_ref().map(ArtifactImportRef::as_str),
    })
}

fn artifact_record(record: &ArtifactRecord) -> Value {
    let mut value = json!({
        "artifact": artifact_ref(&record.artifact),
        "tombstone": record.tombstone.as_ref().map(artifact_tombstone),
    });
    insert_authorization(&mut value, record.authorization.as_ref());
    value
}

fn artifact_tombstone(value: &ArtifactTombstone) -> Value {
    let mut rendered = json!({
        "actor": value.actor.as_str(),
        "policy": value.policy.as_str(),
        "retired_at": format_time(value.retired_at),
        "digest": value.digest.as_str(),
    });
    insert_authorization(&mut rendered, value.authorization.as_ref());
    rendered
}

fn insert_authorization(
    value: &mut Value,
    authorization: Option<&made_core::value_objects::AuthorizationEvidence>,
) {
    if let (Some(object), Some(authorization)) = (value.as_object_mut(), authorization) {
        object.insert("authorization".to_owned(), json!(authorization));
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>, ToolError> {
    value
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))
}

fn required_string<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a str, ToolError> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request(format!("missing required string `{key}`")))
}

fn optional_string<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

fn required_u64(obj: &Map<String, Value>, key: &str) -> Result<u64, ToolError> {
    obj.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::invalid_request(format!("missing required integer `{key}`")))
}

fn optional_u64(obj: &Map<String, Value>, key: &str) -> Result<Option<u64>, ToolError> {
    obj.get(key)
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                ToolError::invalid_request(format!("`{key}` must be an unsigned integer"))
            })
        })
        .transpose()
}

fn parse_time(value: &str) -> Result<OffsetDateTime, ToolError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| ToolError::invalid_request(format!("invalid RFC3339 timestamp: {error}")))
}

pub(in crate::embedded) fn format_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("validated artifact timestamps always format as RFC3339")
}
