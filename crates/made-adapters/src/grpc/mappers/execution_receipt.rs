use made_app::workers::{
    CompleteExecutionReceiptInput, ExecutionRecoveryItem, ExecutionRecoveryItemsPage,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionOperationId, ExecutionReceipt,
    ExecutionReceiptLinkKind, ExecutionRecoveryCapability, ExecutionRecoveryCursor,
    ExecutionRecoveryPageLimit, StepClaimFence, StepId,
};
use made_proto::v1 as pb;

use super::actor_kind::actor_kind_from_proto;
use super::artifact::artifact_ref_to_proto;
use super::attributes::attributes_to_struct;
use super::timestamp::offset_to_timestamp;

pub fn complete_execution_receipt_input_from_proto(
    request: pb::ApplyExecutionReceiptRequest,
    link_kind: ExecutionReceiptLinkKind,
) -> Result<CompleteExecutionReceiptInput, DomainError> {
    Ok(CompleteExecutionReceiptInput {
        ceremony_id: CeremonyId::new(request.ceremony_id)?,
        step_id: StepId::new(request.step_id)?,
        operation_id: ExecutionOperationId::new(request.operation_id)?,
        claim_fence: StepClaimFence::new(request.claim_fence)?,
        link_kind,
        actor_kind: actor_kind_from_proto(&request.actor_kind, "actor_kind")?,
    })
}

pub fn execution_recovery_cursor_from_proto(
    raw: Option<String>,
) -> Result<Option<ExecutionRecoveryCursor>, DomainError> {
    raw.map(ExecutionRecoveryCursor::new).transpose()
}

pub fn execution_recovery_limit_from_proto(
    raw: u32,
) -> Result<ExecutionRecoveryPageLimit, DomainError> {
    if raw == 0 {
        return Ok(ExecutionRecoveryPageLimit::DEFAULT);
    }
    let value = u16::try_from(raw).map_err(|_| DomainError::OutOfRange {
        field: "execution_recovery_page_limit",
        value: f64::from(raw),
        min: 1.0,
        max: f64::from(ExecutionRecoveryPageLimit::MAX),
    })?;
    ExecutionRecoveryPageLimit::new(value)
}

#[must_use]
pub fn execution_receipt_to_proto(receipt: &ExecutionReceipt) -> pb::ExecutionReceipt {
    pb::ExecutionReceipt {
        receipt_id: receipt.receipt_id().as_str().to_owned(),
        operation_id: receipt.operation_id().as_str().to_owned(),
        request_digest: receipt.request_digest().as_str().to_owned(),
        producer_claim_fence: receipt.producer_claim_fence().as_str().to_owned(),
        connector_id: receipt.connector_id().as_str().to_owned(),
        external_operation_id: receipt
            .external_operation_id()
            .map(|id| id.as_str().to_owned()),
        recovery_capability: recovery_capability_label(receipt.recovery_capability()).to_owned(),
        source_kind: source_kind_proto(receipt.source_kind()),
        status: receipt.result().status().as_label().to_owned(),
        output: Some(attributes_to_struct(receipt.result().output().attributes())),
        error: receipt.result().error_message().map(ToString::to_string),
        artifacts: receipt
            .artifacts()
            .iter()
            .map(artifact_ref_to_proto)
            .collect(),
        observed_at: Some(offset_to_timestamp(receipt.observed_at())),
    }
}

#[must_use]
pub fn execution_recovery_page_to_proto(
    page: &ExecutionRecoveryItemsPage,
) -> pb::InspectExecutionRecoveryResponse {
    pb::InspectExecutionRecoveryResponse {
        items: page.items().iter().map(recovery_item_to_proto).collect(),
        next_cursor: page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
    }
}

fn recovery_item_to_proto(item: &ExecutionRecoveryItem) -> pb::ExecutionRecoveryItem {
    let operation = item.operation();
    pb::ExecutionRecoveryItem {
        operation_id: operation.operation_id().as_str().to_owned(),
        ceremony_id: operation.ceremony_id().as_str().to_owned(),
        step_id: operation.step_id().as_str().to_owned(),
        request_digest: operation.request_digest().as_str().to_owned(),
        intents: item
            .intents()
            .iter()
            .map(|intent| pb::ExecutionIntentSummary {
                claim_fence: intent.claim_fence().as_str().to_owned(),
                connector_id: intent.connector_id().as_str().to_owned(),
                recovery_capability: recovery_capability_label(intent.recovery_capability())
                    .to_owned(),
                source_kind: source_kind_proto(intent.source_kind()),
                actor_kind: actor_kind_label(intent.actor_kind()).to_owned(),
                recorded_at: Some(offset_to_timestamp(intent.recorded_at())),
            })
            .collect(),
        receipt: item.receipt().map(execution_receipt_to_proto),
        current_claim_fence: item
            .current_claim_fence()
            .map(|fence| fence.as_str().to_owned()),
    }
}

const fn recovery_capability_label(capability: ExecutionRecoveryCapability) -> &'static str {
    match capability {
        ExecutionRecoveryCapability::IdempotentByOperationId => "idempotent_by_operation_id",
        ExecutionRecoveryCapability::QueryableByOperationId => "queryable_by_operation_id",
        ExecutionRecoveryCapability::ReconciliationRequired => "reconciliation_required",
    }
}

const fn source_kind_proto(kind: ArtifactSourceKind) -> i32 {
    match kind {
        ArtifactSourceKind::ExternalExecution => pb::ArtifactSourceKind::ExternalExecution as i32,
        ArtifactSourceKind::Fixture => pb::ArtifactSourceKind::Fixture as i32,
        ArtifactSourceKind::NoOp => pb::ArtifactSourceKind::NoOp as i32,
        ArtifactSourceKind::GeneratedReport => pb::ArtifactSourceKind::GeneratedReport as i32,
        ArtifactSourceKind::Imported => pb::ArtifactSourceKind::Imported as i32,
    }
}

const fn actor_kind_label(kind: AuditActorKind) -> &'static str {
    match kind {
        AuditActorKind::Human => "human",
        AuditActorKind::Agent => "agent",
        AuditActorKind::Service => "service",
        AuditActorKind::Engine => "engine",
    }
}
