use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::{artifact_ref_to_json, optional_pb_struct_to_json, timestamp_to_rfc3339};

pub(crate) fn execution_receipt_to_json(receipt: &pb::ExecutionReceipt) -> Value {
    let source_kind = match pb::ArtifactSourceKind::try_from(receipt.source_kind)
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
        "receipt_id": receipt.receipt_id,
        "operation_id": receipt.operation_id,
        "request_digest": receipt.request_digest,
        "producer_claim_fence": receipt.producer_claim_fence,
        "connector_id": receipt.connector_id,
        "external_operation_id": receipt.external_operation_id,
        "recovery_capability": receipt.recovery_capability,
        "source_kind": source_kind,
        "status": receipt.status,
        "output": optional_pb_struct_to_json(receipt.output.clone()),
        "error": receipt.error,
        "artifacts": receipt.artifacts.iter().map(|artifact| {
            artifact_ref_to_json(Some(artifact)).unwrap_or(Value::Null)
        }).collect::<Vec<_>>(),
        "observed_at": timestamp_to_rfc3339(receipt.observed_at.as_ref()),
    })
}

pub(crate) fn execution_recovery_page_to_json(
    response: pb::InspectExecutionRecoveryResponse,
) -> Value {
    json!({
        "items": response.items.into_iter().map(recovery_item).collect::<Vec<_>>(),
        "next_cursor": response.next_cursor,
    })
}

fn recovery_item(item: pb::ExecutionRecoveryItem) -> Value {
    json!({
        "operation_id": item.operation_id,
        "ceremony_id": item.ceremony_id,
        "step_id": item.step_id,
        "request_digest": item.request_digest,
        "intents": item.intents.into_iter().map(intent).collect::<Vec<_>>(),
        "receipt": item.receipt.as_ref().map(execution_receipt_to_json),
        "current_claim_fence": item.current_claim_fence,
    })
}

fn intent(intent: pb::ExecutionIntentSummary) -> Value {
    let source_kind = match pb::ArtifactSourceKind::try_from(intent.source_kind)
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
        "claim_fence": intent.claim_fence,
        "connector_id": intent.connector_id,
        "recovery_capability": intent.recovery_capability,
        "source_kind": source_kind,
        "actor_kind": intent.actor_kind,
        "recorded_at": timestamp_to_rfc3339(intent.recorded_at.as_ref()),
    })
}
