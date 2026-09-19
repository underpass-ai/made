use made_app::workers::{CompleteExecutionReceiptInput, ExecutionRecoveryItemsPage};
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionOperationId, ExecutionReceipt,
    ExecutionReceiptLinkKind, ExecutionRecoveryCursor, ExecutionRecoveryPageLimit, StepClaimFence,
    StepId,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Map, Value};

use super::embedded_request_fields::{
    optional_string, optional_u64, required_actor_kind, required_string,
};
use crate::protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedApplyExecutionReceiptRequest {
    input: CompleteExecutionReceiptInput,
}

impl EmbeddedApplyExecutionReceiptRequest {
    pub(super) async fn execute(
        mut self,
        made: &EmbeddedMade,
        link_kind: ExecutionReceiptLinkKind,
    ) -> Result<CeremonyId, ToolError> {
        self.input.link_kind = link_kind;
        let ceremony_id = self.input.ceremony_id.clone();
        if link_kind == ExecutionReceiptLinkKind::Direct {
            made.complete_execution_receipt(self.input).await?;
        } else {
            made.adopt_execution_receipt(self.input).await?;
        }
        Ok(ceremony_id)
    }
}

impl TryFrom<&Value> for EmbeddedApplyExecutionReceiptRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = object(value)?;
        Ok(Self {
            input: CompleteExecutionReceiptInput {
                ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                    .map_err(|error| error.to_string())?,
                step_id: StepId::new(required_string(object, "step_id")?)
                    .map_err(|error| error.to_string())?,
                operation_id: ExecutionOperationId::new(required_string(object, "operation_id")?)
                    .map_err(|error| error.to_string())?,
                claim_fence: StepClaimFence::new(required_string(object, "claim_fence")?)
                    .map_err(|error| error.to_string())?,
                link_kind: ExecutionReceiptLinkKind::Direct,
                actor_kind: required_actor_kind(object, "actor_kind")?,
            },
        })
    }
}

pub(super) fn operation_id(value: &Value) -> Result<ExecutionOperationId, String> {
    let object = object(value)?;
    ExecutionOperationId::new(required_string(object, "operation_id")?)
        .map_err(|error| error.to_string())
}

pub(super) fn recovery_page(
    value: &Value,
) -> Result<(Option<ExecutionRecoveryCursor>, ExecutionRecoveryPageLimit), String> {
    let object = object(value)?;
    let after = optional_string(object, "after")?
        .map(ExecutionRecoveryCursor::new)
        .transpose()
        .map_err(|error| error.to_string())?;
    let raw = optional_u64(object, "limit")?.unwrap_or_default();
    let limit = if raw == 0 {
        ExecutionRecoveryPageLimit::DEFAULT
    } else {
        ExecutionRecoveryPageLimit::new(
            u16::try_from(raw).map_err(|_| "field `limit` exceeds u16".to_owned())?,
        )
        .map_err(|error| error.to_string())?
    };
    Ok((after, limit))
}

#[must_use]
pub(super) fn present_receipt(receipt: &ExecutionReceipt) -> Value {
    json!({
        "receipt_id": receipt.receipt_id().as_str(),
        "operation_id": receipt.operation_id().as_str(),
        "request_digest": receipt.request_digest().as_str(),
        "producer_claim_fence": receipt.producer_claim_fence().as_str(),
        "connector_id": receipt.connector_id().as_str(),
        "external_operation_id": receipt.external_operation_id().map(|id| id.as_str()),
        "recovery_capability": recovery_capability(receipt.recovery_capability()),
        "source_kind": source_kind(receipt.source_kind()),
        "status": receipt.result().status().as_label(),
        "output": receipt.result().output().attributes().as_map(),
        "error": receipt.result().error_message().map(ToString::to_string),
        "artifacts": receipt.artifacts(),
        "observed_at": receipt.observed_at(),
    })
}

#[must_use]
pub(super) fn present_recovery_page(page: &ExecutionRecoveryItemsPage) -> Value {
    json!({
        "items": page.items().iter().map(|item| {
            let operation = item.operation();
            json!({
                "operation_id": operation.operation_id().as_str(),
                "ceremony_id": operation.ceremony_id().as_str(),
                "step_id": operation.step_id().as_str(),
                "request_digest": operation.request_digest().as_str(),
                "intents": item.intents().iter().map(|intent| json!({
                    "claim_fence": intent.claim_fence().as_str(),
                    "connector_id": intent.connector_id().as_str(),
                    "recovery_capability": recovery_capability(intent.recovery_capability()),
                    "source_kind": source_kind(intent.source_kind()),
                    "actor_kind": actor_kind(intent.actor_kind()),
                    "recorded_at": intent.recorded_at(),
                })).collect::<Vec<_>>(),
                "receipt": item.receipt().map(present_receipt),
                "current_claim_fence": item.current_claim_fence().map(|fence| fence.as_str()),
            })
        }).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor().map(|cursor| cursor.as_str()),
    })
}

fn object(value: &Value) -> Result<&Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| "tools/call.arguments must be an object".to_owned())
}

const fn recovery_capability(
    capability: made_core::value_objects::ExecutionRecoveryCapability,
) -> &'static str {
    use made_core::value_objects::ExecutionRecoveryCapability;
    match capability {
        ExecutionRecoveryCapability::IdempotentByOperationId => "idempotent_by_operation_id",
        ExecutionRecoveryCapability::QueryableByOperationId => "queryable_by_operation_id",
        ExecutionRecoveryCapability::ReconciliationRequired => "reconciliation_required",
    }
}

const fn source_kind(kind: ArtifactSourceKind) -> &'static str {
    match kind {
        ArtifactSourceKind::ExternalExecution => "external_execution",
        ArtifactSourceKind::Fixture => "fixture",
        ArtifactSourceKind::NoOp => "no_op",
        ArtifactSourceKind::GeneratedReport => "generated_report",
        ArtifactSourceKind::Imported => "imported",
    }
}

const fn actor_kind(kind: AuditActorKind) -> &'static str {
    match kind {
        AuditActorKind::Human => "human",
        AuditActorKind::Agent => "agent",
        AuditActorKind::Service => "service",
        AuditActorKind::Engine => "engine",
    }
}
