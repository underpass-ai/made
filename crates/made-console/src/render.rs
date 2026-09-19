use made_client::v1::CeremonyInstanceState;
use made_client::v1::{
    ArtifactRecord, BudgetBalance, BudgetLimits, BudgetMeasurement, BudgetQuantities,
    BudgetReservationEstimate, ExecutionIntentSummary, ExecutionReceipt, ExecutionRecoveryItem,
    GetBudgetReportResponse, InspectExecutionRecoveryResponse,
    ListPendingBudgetReservationsResponse,
};
use made_client::{CeremonySearchPage, CeremonyTree, CeremonyTreeNode, ProgressBatch};
use prost_types::{value::Kind, Struct};
use serde_json::{json, Value};

use crate::OutputFormat;

pub fn instance(instance: &CeremonyInstanceState, format: OutputFormat) -> String {
    render(&instance_value(instance), format)
}

pub fn ceremony_page(page: &CeremonySearchPage, format: OutputFormat) -> String {
    let values: Vec<_> = page.instances().iter().map(instance_value).collect();
    render(
        &json!({ "ceremonies": values, "next_cursor": page.next_cursor() }),
        format,
    )
}

pub fn tree(tree: &CeremonyTree, format: OutputFormat) -> String {
    let roots: Vec<_> = tree.roots().iter().map(tree_node_value).collect();
    render(&json!({ "roots": roots }), format)
}

pub fn artifacts(
    artifacts: &[ArtifactRecord],
    next_cursor: Option<&str>,
    format: OutputFormat,
) -> String {
    let values: Vec<_> = artifacts
        .iter()
        .map(|record| {
            let reference = record.artifact.as_ref();
            json!({
                "artifact_id": reference.map(|value| value.artifact_id.as_str()),
                "digest": reference.map(|value| value.digest.as_str()),
                "size_bytes": reference.map(|value| value.size_bytes),
                "media_type": reference.map(|value| value.media_type.as_str()),
                "source_kind": reference
                    .and_then(|value| value.provenance.as_ref())
                    .map(|value| value.source_kind),
                "tombstoned": record.tombstone.is_some(),
            })
        })
        .collect();
    render(
        &json!({ "artifacts": values, "next_cursor": next_cursor }),
        format,
    )
}

pub fn progress(batch: &ProgressBatch, format: OutputFormat) -> String {
    let records: Vec<_> = batch
        .records()
        .iter()
        .map(|record| {
            json!({
                "event_id": record.event_id,
                "event_type": record.event_type,
                "sequence": record.sequence,
                "occurred_at": record.occurred_at,
                "late": record.event_type == "late_step_result_observed",
            })
        })
        .collect();
    render(
        &json!({
            "records": records,
            "resume_after_sequence": batch.checkpoint().after_sequence(),
            "head_sequence": batch.head_sequence(),
            "end_reason": batch.end_reason().as_str_name(),
        }),
        format,
    )
}

pub fn budget_report(report: &GetBudgetReportResponse, format: OutputFormat) -> String {
    let blocked_dimensions = report
        .balance
        .as_ref()
        .map(blocked_dimensions)
        .unwrap_or_default();
    render(
        &json!({
            "account_id": report.account_id,
            "balance": report.balance.as_ref().map(balance_value),
            "admission": {
                "blocked": !blocked_dimensions.is_empty(),
                "blocked_dimensions": blocked_dimensions,
                "explanation": if blocked_dimensions.is_empty() {
                    "no exhausted or overrun bounded dimension"
                } else {
                    "a bounded dimension is exhausted or overrun; inspect pending reservations before retrying"
                },
            },
        }),
        format,
    )
}

pub fn pending_budget_reservations(
    page: &ListPendingBudgetReservationsResponse,
    format: OutputFormat,
) -> String {
    let reservations: Vec<_> = page
        .reservations
        .iter()
        .map(|reservation| {
            json!({
                "reservation_id": reservation.reservation_id,
                "operation_id": reservation.operation_id,
                "quantities": reservation.quantities.as_ref().map(quantities_value),
                "estimate": reservation.estimate.as_ref().map(estimate_value),
                "reserved_at": reservation.reserved_at,
                "reconciled": reservation.reconciled,
            })
        })
        .collect();
    render(
        &json!({
            "reservations": reservations,
            "next_cursor": optional_text(&page.next_cursor),
            "explanation": "pending reservations reduce available budget until terminal reconciliation",
        }),
        format,
    )
}

pub fn execution_receipt(receipt: &ExecutionReceipt, format: OutputFormat) -> String {
    render(&execution_receipt_value(receipt), format)
}

pub fn execution_recovery(page: &InspectExecutionRecoveryResponse, format: OutputFormat) -> String {
    render(
        &json!({
            "items": page.items.iter().map(execution_recovery_item_value).collect::<Vec<_>>(),
            "next_cursor": page.next_cursor,
        }),
        format,
    )
}

pub fn message(value: &Value, format: OutputFormat) -> String {
    render(value, format)
}

pub(crate) fn value(value: &Value, format: OutputFormat) -> String {
    render(value, format)
}

fn instance_value(instance: &CeremonyInstanceState) -> Value {
    let steps: Vec<_> = instance
        .steps
        .iter()
        .map(|step| {
            json!({
                "step_id": step.step_id,
                "state_id": step.state_id,
                "status": step.status,
                "attempt": step.attempt,
                "state_visit": step.state_visit,
                "state_iteration": step.state_iteration,
                "step_iteration": step.iteration,
                "error": optional_text(&step.error),
            })
        })
        .collect();
    json!({
        "ceremony_id": instance.ceremony_id,
        "definition": {
            "name": instance.definition_name,
            "version": instance.definition_version,
        },
        "lifecycle": instance.lifecycle,
        "end_reason": optional_text(&instance.end_reason),
        "completed": instance.completed,
        "current_state": instance.current_state,
        "current_state_visit": instance.current_state_visit,
        "current_state_iteration": instance.current_state_iteration,
        "next_step_id": optional_text(&instance.next_step_id),
        "claimable_step_ids": instance.claimable_step_ids,
        "waiting_for_human": instance.waiting_for_human,
        "open_intervention_ids": instance.open_intervention_ids,
        "paused_at": optional_text(&instance.paused_at),
        "ended_at": optional_text(&instance.ended_at),
        "ceremony_deadline_at": optional_text(&instance.ceremony_deadline_at),
        "state_deadline_at": optional_text(&instance.state_deadline_at),
        "rehydratable": instance.rehydratable,
        "unrehydratable_reason": optional_text(&instance.unrehydratable_reason),
        "budget": "unknown",
        "steps": steps,
    })
}

fn tree_node_value(node: &CeremonyTreeNode) -> Value {
    let children: Vec<_> = node.children().iter().map(tree_node_value).collect();
    json!({
        "ceremony": instance_value(node.instance()),
        "children": children,
    })
}

fn optional_text(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn execution_receipt_value(receipt: &ExecutionReceipt) -> Value {
    json!({
        "receipt_id": receipt.receipt_id,
        "operation_id": receipt.operation_id,
        "request_digest": receipt.request_digest,
        "producer_claim_fence": receipt.producer_claim_fence,
        "connector_id": receipt.connector_id,
        "external_operation_id": receipt.external_operation_id,
        "recovery_capability": receipt.recovery_capability,
        "source_kind": made_client::v1::ArtifactSourceKind::try_from(receipt.source_kind)
            .map_or("unspecified", |kind| kind.as_str_name()),
        "status": receipt.status,
        "output": receipt.output.as_ref().map(struct_value),
        "error": receipt.error,
        "artifacts": receipt.artifacts.iter().map(|artifact| json!({
            "artifact_id": artifact.artifact_id,
            "digest": artifact.digest,
            "size_bytes": artifact.size_bytes,
            "media_type": artifact.media_type,
        })).collect::<Vec<_>>(),
        "observed_at": receipt.observed_at.as_ref().map(timestamp_value),
    })
}

fn execution_recovery_item_value(item: &ExecutionRecoveryItem) -> Value {
    json!({
        "operation_id": item.operation_id,
        "ceremony_id": item.ceremony_id,
        "step_id": item.step_id,
        "request_digest": item.request_digest,
        "intents": item.intents.iter().map(execution_intent_value).collect::<Vec<_>>(),
        "receipt": item.receipt.as_ref().map(execution_receipt_value),
        "current_claim_fence": item.current_claim_fence,
    })
}

fn execution_intent_value(intent: &ExecutionIntentSummary) -> Value {
    json!({
        "claim_fence": intent.claim_fence,
        "connector_id": intent.connector_id,
        "recovery_capability": intent.recovery_capability,
        "source_kind": made_client::v1::ArtifactSourceKind::try_from(intent.source_kind)
            .map_or("unspecified", |kind| kind.as_str_name()),
        "actor_kind": intent.actor_kind,
        "recorded_at": intent.recorded_at.as_ref().map(timestamp_value),
    })
}

fn timestamp_value(timestamp: &prost_types::Timestamp) -> Value {
    json!({"seconds": timestamp.seconds, "nanos": timestamp.nanos})
}

fn struct_value(value: &Struct) -> Value {
    Value::Object(
        value
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), prost_value(value)))
            .collect(),
    )
}

fn prost_value(value: &prost_types::Value) -> Value {
    match value.kind.as_ref() {
        None | Some(Kind::NullValue(_)) => Value::Null,
        Some(Kind::NumberValue(value)) => json!(value),
        Some(Kind::StringValue(value)) => json!(value),
        Some(Kind::BoolValue(value)) => json!(value),
        Some(Kind::StructValue(value)) => struct_value(value),
        Some(Kind::ListValue(value)) => {
            Value::Array(value.values.iter().map(prost_value).collect())
        }
    }
}

fn balance_value(balance: &BudgetBalance) -> Value {
    json!({
        "limits": balance.limits.as_ref().map(limits_value),
        "reserved": balance.reserved.as_ref().map(quantities_value),
        "observed": balance.observed.as_ref().map(quantities_value),
        "estimated": balance.estimated.as_ref().map(quantities_value),
        "unconfirmed": balance.unconfirmed.as_ref().map(quantities_value),
        "overrun": balance.overrun.as_ref().map(quantities_value),
        "available": balance.available.as_ref().map(quantities_value),
    })
}

fn limits_value(limits: &BudgetLimits) -> Value {
    json!({
        "duration_micros": limits.duration_micros,
        "tokens": limits.tokens,
        "cost_micros": limits.cost_micros,
        "tool_calls": limits.tool_calls,
        "currency": limits.currency,
    })
}

fn quantities_value(quantities: &BudgetQuantities) -> Value {
    json!({
        "duration_micros": quantities.duration_micros,
        "tokens": quantities.tokens,
        "cost_micros": quantities.cost_micros,
        "tool_calls": quantities.tool_calls,
    })
}

fn estimate_value(estimate: &BudgetReservationEstimate) -> Value {
    json!({
        "duration": estimate.duration.as_ref().map(measurement_value),
        "tokens": estimate.tokens.as_ref().map(measurement_value),
        "cost": estimate.cost.as_ref().map(measurement_value),
        "tool_calls": estimate.tool_calls.as_ref().map(measurement_value),
    })
}

fn measurement_value(measurement: &BudgetMeasurement) -> Value {
    json!({
        "quality": measurement.quality,
        "amount": measurement.amount,
    })
}

fn blocked_dimensions(balance: &BudgetBalance) -> Vec<&'static str> {
    let Some(limits) = &balance.limits else {
        return Vec::new();
    };
    let available = balance.available.as_ref();
    let overrun = balance.overrun.as_ref();
    let mut blocked = Vec::new();
    if dimension_blocked(
        limits.duration_micros,
        available.map(|value| value.duration_micros),
        overrun.map(|value| value.duration_micros),
    ) {
        blocked.push("duration_micros");
    }
    if dimension_blocked(
        limits.tokens,
        available.map(|value| value.tokens),
        overrun.map(|value| value.tokens),
    ) {
        blocked.push("tokens");
    }
    if dimension_blocked(
        limits.cost_micros,
        available.map(|value| value.cost_micros),
        overrun.map(|value| value.cost_micros),
    ) {
        blocked.push("cost_micros");
    }
    if dimension_blocked(
        limits.tool_calls,
        available.map(|value| value.tool_calls),
        overrun.map(|value| value.tool_calls),
    ) {
        blocked.push("tool_calls");
    }
    blocked
}

fn dimension_blocked(limit: Option<u64>, available: Option<u64>, overrun: Option<u64>) -> bool {
    limit.is_some() && (available == Some(0) || overrun.is_some_and(|value| value > 0))
}

fn render(value: &Value, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => serde_json::to_string_pretty(value).expect("JSON values serialize"),
        OutputFormat::Json => serde_json::to_string(value).expect("JSON values serialize"),
    }
}
