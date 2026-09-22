//! What the engine knows, rendered into the contract's messages.
//!
//! The other half of the loop's mapping: nothing here refuses, because
//! everything here is already a domain value. See
//! `integrator_loop_requests.rs` for the direction that does.

use made_app::services::attention::AttentionEvent;
use made_app::usecases::integrator::{
    AttentionBatch, AttentionContext, AttentionDelivery, IntegratorAttentionAcknowledged,
};
use made_core::ports::{
    AckOutcome, BindOutcome, DeliveryFailureOutcome, HostDeliveryPage, ProcessedOutcome,
};
use made_core::value_objects::{
    AgenticSystemExecutionId, GlobalPosition, HostDeliveryRecord, IntegratorBinding,
    IntegratorScope, ProcessedActionRef, StateId, StepId,
};
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn bind_outcome_to_proto(outcome: &BindOutcome) -> pb::BindCeremonyIntegratorResponse {
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
    pb::BindCeremonyIntegratorResponse {
        outcome: label.to_owned(),
        binding: current.map(integrator_binding_state),
        previous: previous.map(integrator_binding_state),
    }
}

pub fn integrator_binding_state(binding: &IntegratorBinding) -> pb::IntegratorBindingState {
    pb::IntegratorBindingState {
        binding_id: binding.id().as_str().to_owned(),
        scope: Some(integrator_scope_state(binding.scope())),
        role_id: binding.role_id().as_str().to_owned(),
        host_kind: binding.destination().host_kind().as_str().to_owned(),
        address: binding.destination().address().as_str().to_owned(),
        activation: binding.destination().activation().as_str().to_owned(),
        incarnation: binding.incarnation().as_str().to_owned(),
        fence: binding.fence().value(),
        bound_at: rfc3339(binding.bound_at()),
        revoked_at: binding.revoked_at().map(rfc3339).unwrap_or_default(),
    }
}

pub fn attention_batch_to_proto(batch: &AttentionBatch) -> pb::AwaitIntegratorAttentionResponse {
    pb::AwaitIntegratorAttentionResponse {
        items: batch.items().iter().map(attention_delivery_state).collect(),
        loop_state: label(batch.loop_state()),
        end_reason: label(batch.end_reason()),
        journal_head: batch.journal_head().map(GlobalPosition::value),
    }
}

pub fn integrator_acknowledged_to_proto(
    acknowledged: &IntegratorAttentionAcknowledged,
) -> pb::AcknowledgeIntegratorAttentionResponse {
    let (acknowledgement, action, outcome, record, conflict) = match acknowledged {
        IntegratorAttentionAcknowledged::Intent { outcome, action } => {
            let (label, record, conflict) = ack_outcome_parts(outcome);
            ("intent", Some(&**action), label, record, conflict)
        }
        IntegratorAttentionAcknowledged::Processed(outcome) => {
            let (label, record, conflict) = processed_outcome_parts(outcome);
            // The act echoed back is the one the ledger sealed rather
            // than the one the caller sent, so a retry that lost its
            // answer reads the same as the call that won.
            let action = record.and_then(|record| record.state().action());
            ("processed", action, label, record, conflict)
        }
        IntegratorAttentionAcknowledged::Failed(outcome) => {
            let (label, record) = match outcome {
                DeliveryFailureOutcome::Requeued(record) => ("requeued", Some(record)),
                DeliveryFailureOutcome::Exhausted(record) => ("exhausted", Some(record)),
                DeliveryFailureOutcome::LeaseNotOwned => ("lease_not_owned", None),
            };
            ("failed", None, label, record, String::new())
        }
    };
    pb::AcknowledgeIntegratorAttentionResponse {
        acknowledgement: acknowledgement.to_owned(),
        outcome: outcome.to_owned(),
        action_kind: action
            .map(|action| action.kind().as_str().to_owned())
            .unwrap_or_default(),
        idempotency_key: action
            .and_then(ProcessedActionRef::idempotency_key)
            .map(|key| key.as_str().to_owned())
            .unwrap_or_default(),
        delivery: record.map(attention_delivery_record_state),
        conflict,
    }
}

pub fn attention_delivery_page_to_proto(
    page: &HostDeliveryPage,
) -> pb::ListAttentionDeliveriesResponse {
    pb::ListAttentionDeliveriesResponse {
        deliveries: page
            .records()
            .iter()
            .map(attention_delivery_record_state)
            .collect(),
        next_cursor: page
            .next_cursor()
            .map(|cursor| cursor.as_str().to_owned())
            .unwrap_or_default(),
    }
}

fn ack_outcome_parts(outcome: &AckOutcome) -> (&'static str, Option<&HostDeliveryRecord>, String) {
    match outcome {
        AckOutcome::Acknowledged(record) => ("acknowledged", Some(record), String::new()),
        AckOutcome::AlreadyAcknowledged(record) => {
            ("already_acknowledged", Some(record), String::new())
        }
        AckOutcome::Conflict { existing } => (
            "conflict",
            None,
            format!("already observed as {}", existing.kind()),
        ),
        AckOutcome::LeaseNotOwned => ("lease_not_owned", None, String::new()),
    }
}

fn processed_outcome_parts(
    outcome: &ProcessedOutcome,
) -> (&'static str, Option<&HostDeliveryRecord>, String) {
    match outcome {
        ProcessedOutcome::Processed(record) => ("processed", Some(record), String::new()),
        ProcessedOutcome::AlreadyProcessed(record) => {
            ("already_processed", Some(record), String::new())
        }
        ProcessedOutcome::Conflict { existing } => (
            "conflict",
            None,
            format!("already closed by {}", existing.kind()),
        ),
        ProcessedOutcome::NotAcknowledged { state } => {
            ("not_acknowledged", None, format!("the delivery is {state}"))
        }
        ProcessedOutcome::FenceRejected { current } => (
            "fence_rejected",
            None,
            format!("the binding is at fence {current}"),
        ),
        ProcessedOutcome::LeaseNotOwned => ("lease_not_owned", None, String::new()),
    }
}

fn attention_delivery_state(delivery: &AttentionDelivery) -> pb::AttentionDeliveryState {
    pb::AttentionDeliveryState {
        delivery_id: delivery.lease().delivery_id().as_str().to_owned(),
        lease_id: delivery.lease().lease_id().as_str().to_owned(),
        leased_until: rfc3339(delivery.leased_until()),
        attention: Some(attention_event_state(delivery.attention())),
        context: Some(attention_context_state(delivery.context())),
    }
}

fn attention_event_state(attention: &AttentionEvent) -> pb::AttentionEventState {
    pb::AttentionEventState {
        attention_id: attention.id().as_str().to_owned(),
        kind: attention.kind().as_str().to_owned(),
        ceremony_id: attention.ceremony_id().as_str().to_owned(),
        system_execution_id: attention
            .system_execution_id()
            .map(AgenticSystemExecutionId::as_str)
            .unwrap_or_default()
            .to_owned(),
        step_id: attention
            .step_id()
            .map(StepId::as_str)
            .unwrap_or_default()
            .to_owned(),
        position: attention.position().value(),
        occurred_at: rfc3339(attention.occurred_at()),
        reason: attention.reason().as_str().to_owned(),
        evidence: attention
            .evidence()
            .iter()
            .map(|reference| reference.as_str().to_owned())
            .collect(),
        acceptance: label(attention.acceptance()),
        source_event_id: attention.source().event_id().as_str().to_owned(),
        source_sequence: attention.source().sequence().value(),
    }
}

fn attention_context_state(context: &AttentionContext) -> pb::AttentionContextState {
    pb::AttentionContextState {
        ceremony_id: context.ceremony_id().as_str().to_owned(),
        lifecycle: label(context.lifecycle()),
        current_state: context
            .current_state()
            .map(StateId::as_str)
            .unwrap_or_default()
            .to_owned(),
        claimable_step_ids: context
            .claimable_step_ids()
            .iter()
            .map(|step| step.as_str().to_owned())
            .collect(),
        waiting_for_human: context
            .waiting_for_human()
            .iter()
            .map(|guard| guard.as_str().to_owned())
            .collect(),
    }
}

fn attention_delivery_record_state(
    record: &HostDeliveryRecord,
) -> pb::AttentionDeliveryRecordState {
    let action = record.state().action();
    pb::AttentionDeliveryRecordState {
        delivery_id: record.id().as_str().to_owned(),
        binding_id: record
            .target()
            .binding_id()
            .map(|binding| binding.as_str().to_owned())
            .unwrap_or_default(),
        ceremony_id: record.item().ceremony_id().as_str().to_owned(),
        attention_id: record.item().item_id().to_owned(),
        state: record.state().kind().as_str().to_owned(),
        attempt: record.attempt().value(),
        lease_id: record
            .state()
            .lease()
            .map(|lease| lease.lease_id().as_str().to_owned())
            .unwrap_or_default(),
        leased_until: record
            .state()
            .lease()
            .map(|lease| rfc3339(lease.leased_until()))
            .unwrap_or_default(),
        action_kind: action
            .map(|action| action.kind().as_str().to_owned())
            .unwrap_or_default(),
        idempotency_key: action
            .and_then(ProcessedActionRef::idempotency_key)
            .map(|key| key.as_str().to_owned())
            .unwrap_or_default(),
        failure_reason: record
            .state()
            .failure_reason()
            .map(|reason| reason.as_str().to_owned())
            .unwrap_or_default(),
        created_at: rfc3339(record.created_at()),
        updated_at: rfc3339(record.updated_at()),
    }
}

fn integrator_scope_state(scope: &IntegratorScope) -> pb::IntegratorScopeState {
    pb::IntegratorScopeState {
        kind: match scope {
            IntegratorScope::Ceremony { .. } => "ceremony".to_owned(),
            IntegratorScope::SystemExecution { .. } => "system_execution".to_owned(),
        },
        ceremony_id: scope
            .ceremony_id()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
        system_execution_id: scope
            .system_execution_id()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
    }
}

fn label(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_else(|_| at.to_string())
}
