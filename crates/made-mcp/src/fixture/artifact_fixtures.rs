use serde_json::{json, Value};

/// Which tools this family answers.
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_begin_artifact_upload"
            | "made_put_artifact_chunk"
            | "made_commit_artifact_upload"
            | "made_abort_artifact_upload"
            | "made_get_artifact"
            | "made_list_artifacts"
            | "made_read_artifact_chunk"
            | "made_tombstone_artifact"
    )
}

/// One arm per tool rather than a combined pattern: the catalog gate
/// scans this source for each tool's own arm, and a tool folded into
/// a neighbour's pattern would read as one nothing answers.
pub(super) fn response(name: &str) -> Value {
    match name {
        "made_begin_artifact_upload" => upload(),
        "made_put_artifact_chunk" => upload(),
        "made_commit_artifact_upload" => reference(),
        "made_abort_artifact_upload" => json!({ "aborted": true }),
        "made_get_artifact" => record(),
        "made_list_artifacts" => listing(),
        "made_read_artifact_chunk" => chunk(),
        "made_tombstone_artifact" => tombstone(),
        _ => unreachable!("only artifact tools reach this fixture"),
    }
}

pub(super) fn upload() -> Value {
    json!({ "upload_id": "upload-fixture-1", "next_offset": 0, "chunk_limit": 65536 })
}

pub(super) fn reference() -> Value {
    json!({
        "artifact_id": "artifact-fixture-1",
        "digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
        "size_bytes": 7,
        "media_type": "text/plain",
        "provenance": {
            "source_kind": "fixture",
            "execution_receipt_id": "receipt-fixture-1",
            "operation_id": "operation-fixture-1",
            "accepted_claim_fence": "claim-fixture-1",
            "observed_at": "2026-01-01T00:00:00Z",
            "import_ref": Value::Null,
        }
    })
}

pub(super) fn record() -> Value {
    json!({ "artifact": reference(), "tombstone": Value::Null })
}

pub(super) fn listing() -> Value {
    json!({ "artifacts": [record()], "next_cursor": Value::Null })
}

pub(super) fn chunk() -> Value {
    json!({
        "bytes_base64": "Zml4dHVyZQ==",
        "next_offset": 7,
        "chunk_digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
        "eof": true,
    })
}

pub(super) fn tombstone() -> Value {
    json!({
        "actor": "fixture-host",
        "policy": "fixture-retention",
        "retired_at": "2026-01-01T00:00:00Z",
        "digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
    })
}
