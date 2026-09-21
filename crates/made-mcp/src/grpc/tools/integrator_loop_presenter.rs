//! What the loop looks like on the wire.
//!
//! Its own file because the embedded backend renders the same five
//! shapes from domain types, and the parity gate holds the two to one
//! answer: a caller should not have to know which engine served it to
//! know where to look.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

fn optional(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

pub(super) fn bound(response: pb::BindCeremonyIntegratorResponse) -> Value {
    json!({
        "outcome": response.outcome,
        "binding": response.binding.map(binding_state),
        "previous": response.previous.map(binding_state),
    })
}

pub(super) fn one_binding(state: Option<pb::IntegratorBindingState>) -> Value {
    json!({ "binding": state.map(binding_state) })
}

pub(super) fn batch(response: pb::AwaitIntegratorAttentionResponse) -> Value {
    json!({
        "items": response.items.into_iter().map(delivery).collect::<Vec<_>>(),
        "loop_state": response.loop_state,
        "end_reason": response.end_reason,
    })
}

pub(super) fn acknowledged(response: pb::AcknowledgeIntegratorAttentionResponse) -> Value {
    json!({
        "acknowledgement": response.acknowledgement,
        "outcome": response.outcome,
        "action_kind": optional(response.action_kind),
        "idempotency_key": optional(response.idempotency_key),
        "delivery": response.delivery.map(record),
        "conflict": optional(response.conflict),
    })
}

pub(super) fn page(
    deliveries: Vec<pb::AttentionDeliveryRecordState>,
    next_cursor: String,
) -> Value {
    json!({
        "deliveries": deliveries.into_iter().map(record).collect::<Vec<_>>(),
        "next_cursor": optional(next_cursor),
    })
}

fn binding_state(state: pb::IntegratorBindingState) -> Value {
    json!({
        "binding_id": state.binding_id,
        "scope": state.scope.as_ref().map(scope),
        "role_id": state.role_id,
        "host_kind": state.host_kind,
        "address": state.address,
        "activation": state.activation,
        "incarnation": state.incarnation,
        "fence": state.fence,
        "bound_at": state.bound_at,
        "revoked_at": optional(state.revoked_at),
    })
}

/// A scope names what it names; the id it does not use is absent
/// rather than an empty string.
fn scope(state: &pb::IntegratorScopeState) -> Value {
    let mut value = json!({ "kind": state.kind.clone() });
    let object = value.as_object_mut().expect("a scope renders as an object");
    if !state.ceremony_id.is_empty() {
        object.insert("ceremony_id".to_owned(), json!(state.ceremony_id.clone()));
    }
    if !state.system_execution_id.is_empty() {
        object.insert(
            "system_execution_id".to_owned(),
            json!(state.system_execution_id.clone()),
        );
    }
    value
}

fn delivery(state: pb::AttentionDeliveryState) -> Value {
    json!({
        "delivery_id": state.delivery_id,
        "lease_id": state.lease_id,
        "leased_until": state.leased_until,
        "attention": state.attention.map(attention),
        "context": state.context.map(context),
    })
}

fn attention(state: pb::AttentionEventState) -> Value {
    json!({
        "attention_id": state.attention_id,
        "kind": state.kind.clone(),
        "ceremony_id": state.ceremony_id,
        "system_execution_id": optional(state.system_execution_id),
        "step_id": optional(state.step_id),
        "position": state.position,
        "occurred_at": state.occurred_at,
        "reason": state.reason,
        "evidence": state.evidence,
        "acceptance": state.acceptance,
        "source_event_id": state.source_event_id,
        "source_sequence": state.source_sequence,
    })
}

fn context(state: pb::AttentionContextState) -> Value {
    json!({
        "ceremony_id": state.ceremony_id,
        "lifecycle": state.lifecycle,
        "current_state": optional(state.current_state),
        "claimable_step_ids": state.claimable_step_ids,
        "waiting_for_human": state.waiting_for_human,
    })
}

fn record(state: pb::AttentionDeliveryRecordState) -> Value {
    json!({
        "delivery_id": state.delivery_id,
        "binding_id": optional(state.binding_id),
        "ceremony_id": state.ceremony_id,
        "attention_id": state.attention_id,
        "state": state.state,
        "attempt": state.attempt,
        "lease_id": optional(state.lease_id),
        "leased_until": optional(state.leased_until),
        "action_kind": optional(state.action_kind),
        "idempotency_key": optional(state.idempotency_key),
        "failure_reason": optional(state.failure_reason),
        "created_at": state.created_at,
        "updated_at": state.updated_at,
    })
}
