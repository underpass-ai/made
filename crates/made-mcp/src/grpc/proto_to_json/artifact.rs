use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::timestamp_to_rfc3339;

pub(crate) fn artifact_upload_status_to_json(
    status: Option<pb::ArtifactUploadStatus>,
) -> Result<Value, crate::protocol::ToolError> {
    let status = status.ok_or_else(|| missing("artifact upload status"))?;
    Ok(json!({
        "upload_id": status.upload_id,
        "next_offset": status.next_offset,
        "chunk_limit": status.chunk_limit,
    }))
}

pub(crate) fn artifact_ref_to_json(
    artifact: Option<&pb::ArtifactReference>,
) -> Result<Value, crate::protocol::ToolError> {
    artifact
        .map(artifact_ref)
        .ok_or_else(|| missing("artifact reference"))
}

pub(crate) fn artifact_record_to_json(
    record: Option<&pb::ArtifactRecord>,
) -> Result<Value, crate::protocol::ToolError> {
    record
        .map(artifact_record)
        .ok_or_else(|| missing("artifact record"))
}

pub(crate) fn artifact_tombstone_to_json(
    tombstone: Option<&pb::ArtifactTombstoneRecord>,
) -> Result<Value, crate::protocol::ToolError> {
    tombstone
        .map(artifact_tombstone)
        .ok_or_else(|| missing("artifact tombstone"))
}

pub(crate) fn artifact_listing_to_json(response: &pb::ListArtifactsResponse) -> Value {
    json!({
        "artifacts": response.artifacts.iter().map(artifact_record).collect::<Vec<_>>(),
        "next_cursor": response.next_cursor,
    })
}

pub(crate) fn artifact_chunk_to_json(response: pb::ReadArtifactChunkResponse) -> Value {
    json!({
        "bytes_base64": STANDARD.encode(response.bytes),
        "next_offset": response.next_offset,
        "chunk_digest": response.chunk_digest,
        "eof": response.eof,
    })
}

fn artifact_ref(artifact: &pb::ArtifactReference) -> Value {
    json!({
        "artifact_id": artifact.artifact_id,
        "digest": artifact.digest,
        "size_bytes": artifact.size_bytes,
        "media_type": artifact.media_type,
        "provenance": artifact.provenance.as_ref().map(provenance),
    })
}

fn provenance(value: &pb::ArtifactProvenance) -> Value {
    let source_kind = match pb::ArtifactSourceKind::try_from(value.source_kind)
        .unwrap_or(pb::ArtifactSourceKind::Unspecified)
    {
        pb::ArtifactSourceKind::ExternalExecution => "external_execution",
        pb::ArtifactSourceKind::Fixture => "fixture",
        pb::ArtifactSourceKind::NoOp => "no_op",
        pb::ArtifactSourceKind::GeneratedReport => "generated_report",
        pb::ArtifactSourceKind::Imported => "imported",
        pb::ArtifactSourceKind::Unspecified => "unspecified",
    };
    json!({
        "source_kind": source_kind,
        "execution_receipt_id": value.execution_receipt_id,
        "operation_id": value.operation_id,
        "accepted_claim_fence": value.accepted_claim_fence,
        "observed_at": timestamp_to_rfc3339(value.observed_at.as_ref()),
        "import_ref": value.import_ref,
    })
}

fn artifact_record(record: &pb::ArtifactRecord) -> Value {
    json!({
        "artifact": record.artifact.as_ref().map(artifact_ref),
        "tombstone": record.tombstone.as_ref().map(artifact_tombstone),
    })
}

fn artifact_tombstone(tombstone: &pb::ArtifactTombstoneRecord) -> Value {
    json!({
        "actor": tombstone.actor,
        "policy": tombstone.policy,
        "retired_at": timestamp_to_rfc3339(tombstone.retired_at.as_ref()),
        "digest": tombstone.digest,
    })
}

fn missing(what: &str) -> crate::protocol::ToolError {
    crate::protocol::ToolError::refused(format!("made returned no {what}"))
}
