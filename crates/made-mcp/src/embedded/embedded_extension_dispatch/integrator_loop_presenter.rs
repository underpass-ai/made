//! What the integrator loop looks like on the wire.
//!
//! Its own file because the gRPC backend renders the same five shapes
//! out of the contract's messages, and the parity gate holds the two
//! to one answer: a presenter living inside a dispatcher would be
//! copied into the next one and then drift from it.

use made_app::services::attention::AttentionEvent;
use made_app::usecases::integrator::{
    AttentionBatch, AttentionContext, AttentionDelivery, IntegratorAttentionAcknowledged,
};
use made_core::ports::{AckOutcome, BindOutcome, DeliveryFailureOutcome, ProcessedOutcome};
use made_core::value_objects::{
    AgenticSystemExecutionId, EvidenceReference, HostDeliveryRecord, IntegratorBinding,
    IntegratorScope, ProcessedActionRef, StateId, StepId,
};
use serde_json::{json, Value};

use super::intervention_delivery::rfc3339;

pub(super) fn present_bind(outcome: &BindOutcome) -> Value {
    let (label, current, previous) = match outcome {
        BindOutcome::Bound(binding) => ("bound", Some(binding), None),
        BindOutcome::AlreadyBound(binding) => ("already_bound", Some(binding), None),
        BindOutcome::Replaced { previous, current } => {
            ("replaced", Some(&**current), Some(&**previous))
        }
        // Nothing was bound. The live binding travels as `previous` so
        // the caller can see who holds the scope it asked for.
        BindOutcome::AlreadyExists { existing } => ("already_exists", None, Some(&**existing)),
    };
    json!({
        "outcome": label,
        "binding": current.map(present_binding),
        "previous": previous.map(present_binding),
    })
}

pub(super) fn present_binding_answer(binding: Option<&IntegratorBinding>) -> Value {
    json!({ "binding": binding.map(present_binding) })
}

pub(super) fn present_batch(batch: &AttentionBatch) -> Value {
    json!({
        "items": batch.items().iter().map(present_delivery).collect::<Vec<_>>(),
        "loop_state": batch.loop_state(),
        "end_reason": batch.end_reason(),
    })
}

pub(super) fn present_acknowledged(acknowledged: &IntegratorAttentionAcknowledged) -> Value {
    match acknowledged {
        IntegratorAttentionAcknowledged::Intent { outcome, action } => {
            let (label, record, conflict) = ack_parts(outcome);
            answer("intent", label, Some(action), record, conflict.as_deref())
        }
        IntegratorAttentionAcknowledged::Processed(outcome) => {
            let (label, record, conflict) = processed_parts(outcome);
            answer(
                "processed",
                label,
                record.and_then(|record| record.state().action()),
                record,
                conflict.as_deref(),
            )
        }
        IntegratorAttentionAcknowledged::Failed(outcome) => {
            let (label, record) = match outcome {
                DeliveryFailureOutcome::Requeued(record) => ("requeued", Some(record)),
                DeliveryFailureOutcome::Exhausted(record) => ("exhausted", Some(record)),
                DeliveryFailureOutcome::LeaseNotOwned => ("lease_not_owned", None),
            };
            answer("failed", label, None, record, None)
        }
    }
}

pub(super) fn present_page(records: &[HostDeliveryRecord], next_cursor: Option<&str>) -> Value {
    json!({
        "deliveries": records.iter().map(present_record).collect::<Vec<_>>(),
        "next_cursor": next_cursor,
    })
}

fn answer(
    acknowledgement: &str,
    outcome: &str,
    action: Option<&ProcessedActionRef>,
    record: Option<&HostDeliveryRecord>,
    conflict: Option<&str>,
) -> Value {
    json!({
        "acknowledgement": acknowledgement,
        "outcome": outcome,
        "action_kind": action.map(|action| action.kind().as_str()),
        "idempotency_key": action
            .and_then(ProcessedActionRef::idempotency_key)
            .map(made_core::value_objects::IdempotencyKey::as_str),
        "delivery": record.map(present_record),
        "conflict": conflict,
    })
}

fn ack_parts(outcome: &AckOutcome) -> (&'static str, Option<&HostDeliveryRecord>, Option<String>) {
    match outcome {
        AckOutcome::Acknowledged(record) => ("acknowledged", Some(record), None),
        AckOutcome::AlreadyAcknowledged(record) => ("already_acknowledged", Some(record), None),
        AckOutcome::Conflict { existing } => (
            "conflict",
            None,
            Some(format!("already observed as {}", existing.kind())),
        ),
        AckOutcome::LeaseNotOwned => ("lease_not_owned", None, None),
    }
}

fn processed_parts(
    outcome: &ProcessedOutcome,
) -> (&'static str, Option<&HostDeliveryRecord>, Option<String>) {
    match outcome {
        ProcessedOutcome::Processed(record) => ("processed", Some(record), None),
        ProcessedOutcome::AlreadyProcessed(record) => ("already_processed", Some(record), None),
        ProcessedOutcome::Conflict { existing } => (
            "conflict",
            None,
            Some(format!("already closed by {}", existing.kind())),
        ),
        ProcessedOutcome::NotAcknowledged { state } => (
            "not_acknowledged",
            None,
            Some(format!("the delivery is {state}")),
        ),
        ProcessedOutcome::FenceRejected { current } => (
            "fence_rejected",
            None,
            Some(format!("the binding is at fence {current}")),
        ),
        ProcessedOutcome::LeaseNotOwned => ("lease_not_owned", None, None),
    }
}

fn present_binding(binding: &IntegratorBinding) -> Value {
    json!({
        "binding_id": binding.id().as_str(),
        "scope": present_scope(binding.scope()),
        "role_id": binding.role_id().as_str(),
        "host_kind": binding.destination().host_kind().as_str(),
        "address": binding.destination().address().as_str(),
        "activation": binding.destination().activation().as_str(),
        "incarnation": binding.incarnation().as_str(),
        "fence": binding.fence().value(),
        "bound_at": rfc3339(binding.bound_at()),
        "revoked_at": binding.revoked_at().map(rfc3339),
    })
}

/// A scope names what it names; the id it does not use is absent
/// rather than null, which is the rule the instance projection follows.
fn present_scope(scope: &IntegratorScope) -> Value {
    let mut value = json!({
        "kind": match scope {
            IntegratorScope::Ceremony { .. } => "ceremony",
            IntegratorScope::SystemExecution { .. } => "system_execution",
        }
    });
    let object = value.as_object_mut().expect("a scope renders as an object");
    if let Some(ceremony_id) = scope.ceremony_id() {
        object.insert("ceremony_id".to_owned(), json!(ceremony_id.as_str()));
    }
    if let Some(execution_id) = scope.system_execution_id() {
        object.insert(
            "system_execution_id".to_owned(),
            json!(execution_id.as_str()),
        );
    }
    value
}

fn present_delivery(delivery: &AttentionDelivery) -> Value {
    json!({
        "delivery_id": delivery.lease().delivery_id().as_str(),
        "lease_id": delivery.lease().lease_id().as_str(),
        "leased_until": rfc3339(delivery.leased_until()),
        "attention": present_attention(delivery.attention()),
        "context": present_context(delivery.context()),
    })
}

fn present_attention(attention: &AttentionEvent) -> Value {
    json!({
        "attention_id": attention.id().as_str(),
        "kind": attention.kind().as_str(),
        "ceremony_id": attention.ceremony_id().as_str(),
        "system_execution_id": attention
            .system_execution_id()
            .map(AgenticSystemExecutionId::as_str),
        "step_id": attention.step_id().map(StepId::as_str),
        "position": attention.position().value(),
        "occurred_at": rfc3339(attention.occurred_at()),
        "reason": attention.reason().as_str(),
        "evidence": attention
            .evidence()
            .iter()
            .map(EvidenceReference::as_str)
            .collect::<Vec<_>>(),
        "acceptance": attention.acceptance(),
        "source_event_id": attention.source().event_id().as_str(),
        "source_sequence": attention.source().sequence().value(),
    })
}

fn present_context(context: &AttentionContext) -> Value {
    json!({
        "ceremony_id": context.ceremony_id().as_str(),
        "lifecycle": context.lifecycle().as_label(),
        "current_state": context.current_state().map(StateId::as_str),
        "claimable_step_ids": context
            .claimable_step_ids()
            .iter()
            .map(StepId::as_str)
            .collect::<Vec<_>>(),
        "waiting_for_human": context
            .waiting_for_human()
            .iter()
            .map(made_core::value_objects::GuardName::as_str)
            .collect::<Vec<_>>(),
    })
}

fn present_record(record: &HostDeliveryRecord) -> Value {
    let action = record.state().action();
    json!({
        "delivery_id": record.id().as_str(),
        "binding_id": record
            .target()
            .binding_id()
            .map(made_core::value_objects::IntegratorBindingId::as_str),
        "ceremony_id": record.item().ceremony_id().as_str(),
        "attention_id": record.item().item_id(),
        "state": record.state().kind().as_str(),
        "attempt": record.attempt().value(),
        "lease_id": record.state().lease().map(|lease| lease.lease_id().as_str()),
        "leased_until": record.state().lease().map(|lease| rfc3339(lease.leased_until())),
        "action_kind": action.map(|action| action.kind().as_str()),
        "idempotency_key": action
            .and_then(ProcessedActionRef::idempotency_key)
            .map(made_core::value_objects::IdempotencyKey::as_str),
        "failure_reason": record
            .state()
            .failure_reason()
            .map(made_core::value_objects::DeliveryFailureReason::as_str),
        "created_at": rfc3339(record.created_at()),
        "updated_at": rfc3339(record.updated_at()),
    })
}
