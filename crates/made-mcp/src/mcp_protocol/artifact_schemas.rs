use serde_json::{json, Value};

use super::schema_primitives::string_schema;

fn provenance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["source_kind", "observed_at"],
        "oneOf": [
            { "required": ["execution_receipt_id", "operation_id", "accepted_claim_fence"] },
            { "required": ["import_ref"] },
            { "properties": { "source_kind": { "const": "generated_report" } } }
        ],
        "properties": {
            "source_kind": {
                "type": "string",
                "enum": ["external_execution", "fixture", "no_op", "generated_report", "imported"]
            },
            "execution_receipt_id": string_schema("Required for execution, fixture, and no-op sources."),
            "operation_id": string_schema("Required for execution, fixture, and no-op sources."),
            "accepted_claim_fence": string_schema("Claim fence accepted by the execution receipt."),
            "observed_at": string_schema("RFC 3339 observation time."),
            "import_ref": string_schema("Required only for imported artifacts.")
        }
    })
}

pub(super) fn begin_artifact_upload_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["expected_digest", "size_bytes", "media_type", "provenance", "idempotency_key"],
        "properties": {
            "requested_artifact_id": string_schema("Optional stable artifact identity; never a path."),
            "expected_digest": string_schema("Canonical sha256:<64 lowercase hex> digest."),
            "size_bytes": { "type": "integer", "minimum": 0, "maximum": 1_073_741_824_u64 },
            "media_type": string_schema("Canonical MIME type without parameters."),
            "provenance": provenance_schema(),
            "idempotency_key": string_schema("Caller-selected replay key for this upload metadata.")
        }
    })
}

pub(super) fn put_artifact_chunk_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["upload_id", "offset", "bytes_base64", "chunk_digest"],
        "properties": {
            "upload_id": string_schema("Upload returned by begin."),
            "offset": { "type": "integer", "minimum": 0 },
            "bytes_base64": string_schema("Base64 bytes; decoded size is bounded by the advertised chunk limit and 1 MiB hard maximum."),
            "chunk_digest": string_schema("SHA-256 digest of this decoded chunk.")
        }
    })
}

pub(super) fn artifact_upload_id_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["upload_id"],
        "properties": { "upload_id": string_schema("Opaque upload identity.") }
    })
}

pub(super) fn artifact_id_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["artifact_id"],
        "properties": { "artifact_id": string_schema("Opaque artifact identity.") }
    })
}

pub(super) fn list_artifacts_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "cursor": string_schema("Opaque cursor returned by the previous page."),
            "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
        }
    })
}

pub(super) fn read_artifact_chunk_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["artifact_id", "offset"],
        "properties": {
            "artifact_id": string_schema("Opaque artifact identity."),
            "offset": { "type": "integer", "minimum": 0 },
            "max_bytes": { "type": "integer", "minimum": 1, "maximum": 1_048_576, "description": "Defaults to 65536." }
        }
    })
}

pub(super) fn tombstone_artifact_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["artifact_id", "actor", "policy", "retired_at"],
        "properties": {
            "artifact_id": string_schema("Opaque artifact identity."),
            "actor": string_schema("Host-asserted retention actor; authorization remains the host boundary."),
            "policy": string_schema("Named retention policy."),
            "retired_at": string_schema("RFC 3339 retention time.")
        }
    })
}
