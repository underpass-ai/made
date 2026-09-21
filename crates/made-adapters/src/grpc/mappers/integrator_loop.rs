//! Between the wire and the five use cases of the integrator loop.
//!
//! One direction reads what a host claims about itself and refuses what
//! it cannot; the other renders what the engine knows. They are in one
//! file because they are one contract, and splitting them would let the
//! two drift a field at a time.

use made_app::services::attention::AttentionEvent;
use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionInput, AttentionBatch, AttentionContext, AttentionDelivery,
    AwaitIntegratorAttentionInput, BindCeremonyIntegratorInput, IntegratorAcknowledgement,
    IntegratorAttentionAcknowledged, ListAttentionDeliveriesInput, DEFAULT_LEASE, DEFAULT_WAIT,
};
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, BindOutcome, BindReplacement, DeliveryFailureOutcome, HostDeliveryPage,
    HostDeliveryPageLimit, ProcessedOutcome,
};
use made_core::value_objects::{
    AgenticSystemExecutionId, CeremonyId, DeliveryFailureReason, DeliveryNote, DurationMs,
    EvidenceReference, HostActivationMode, HostAddress, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryRecord, HostDeliveryStateKind,
    HostDestination, HostKind, IdempotencyKey, IntegratorBinding, IntegratorBindingId,
    IntegratorFence, IntegratorScope, ProcessedActionKind, ProcessedActionRef, RoleId, StateId,
    StepId,
};
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn bind_ceremony_integrator_input_from_proto(
    request: pb::BindCeremonyIntegratorRequest,
) -> Result<BindCeremonyIntegratorInput, DomainError> {
    Ok(BindCeremonyIntegratorInput {
        binding_id: IntegratorBindingId::new(request.binding_id)?,
        scope: scope_from_proto(request.scope)?,
        role_id: RoleId::new(request.role_id)?,
        destination: HostDestination::new(
            HostKind::new(request.host_kind)?,
            HostAddress::new(request.address)?,
            activation_from_proto(&request.activation)?,
        ),
        incarnation: HostAgentIncarnation::new(request.incarnation)?,
        replacement: if request.replace {
            BindReplacement::Replace
        } else {
            BindReplacement::Refuse
        },
        follow: if request.follow_replacement {
            made_core::value_objects::FollowReplacement::Follow
        } else {
            made_core::value_objects::FollowReplacement::Stay
        },
    })
}

pub fn integrator_scope_from_proto(
    scope: Option<pb::IntegratorScopeState>,
) -> Result<IntegratorScope, DomainError> {
    scope_from_proto(scope)
}

pub fn await_integrator_attention_input_from_proto(
    request: pb::AwaitIntegratorAttentionRequest,
) -> Result<AwaitIntegratorAttentionInput, DomainError> {
    Ok(AwaitIntegratorAttentionInput {
        scope: scope_from_proto(request.scope)?,
        binding_id: IntegratorBindingId::new(request.binding_id)?,
        incarnation: HostAgentIncarnation::new(request.incarnation)?,
        fence: fence_from_proto(request.fence),
        limit: page_limit(request.limit)?,
        // Zero is "say nothing about waiting", not "do not wait": a
        // host that omitted the field still wants the short hold the
        // schema promises it.
        wait: if request.wait_timeout_ms == 0 {
            DEFAULT_WAIT
        } else {
            DurationMs::from_millis(request.wait_timeout_ms)
        },
        lease_duration: if request.lease_duration_ms == 0 {
            DEFAULT_LEASE
        } else {
            DurationMs::from_millis(request.lease_duration_ms)
        },
    })
}

pub fn acknowledge_integrator_attention_input_from_proto(
    request: pb::AcknowledgeIntegratorAttentionRequest,
) -> Result<AcknowledgeIntegratorAttentionInput, DomainError> {
    let incarnation = HostAgentIncarnation::new(request.incarnation)?;
    let delivery_id = HostDeliveryId::new(request.delivery_id)?;
    Ok(AcknowledgeIntegratorAttentionInput {
        binding_id: IntegratorBindingId::new(request.binding_id)?,
        incarnation: incarnation.clone(),
        fence: fence_from_proto(request.fence),
        delivery_id: delivery_id.clone(),
        lease: HostDeliveryLease::new(
            delivery_id,
            HostDeliveryLeaseId::new(request.lease_id)?,
            incarnation,
            // The ledger compares the lease identifier, not this
            // instant: a host repeating itself does not get to extend
            // its own hold by saying so.
            OffsetDateTime::now_utc(),
        ),
        outcome: acknowledgement_from_proto(
            &request.acknowledgement,
            &request.action_kind,
            &request.idempotency_key,
            &request.note,
            &request.evidence,
            &request.failure_reason,
        )?,
    })
}

pub fn list_attention_deliveries_input_from_proto(
    request: pb::ListAttentionDeliveriesRequest,
) -> Result<ListAttentionDeliveriesInput, DomainError> {
    Ok(ListAttentionDeliveriesInput {
        binding_id: optional(request.binding_id)
            .map(IntegratorBindingId::new)
            .transpose()?,
        ceremony_id: optional(request.ceremony_id)
            .map(CeremonyId::new)
            .transpose()?,
        state: optional(request.state)
            .map(|raw| delivery_state_from_proto(&raw))
            .transpose()?,
        limit: page_limit(request.limit)?,
        cursor: optional(request.cursor)
            .map(HostDeliveryId::new)
            .transpose()?,
    })
}

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
    }
}

pub fn integrator_acknowledged_to_proto(
    acknowledged: &IntegratorAttentionAcknowledged,
) -> pb::AcknowledgeIntegratorAttentionResponse {
    let mut response = pb::AcknowledgeIntegratorAttentionResponse::default();
    match acknowledged {
        IntegratorAttentionAcknowledged::Intent { outcome, action } => {
            response.acknowledgement = "intent".to_owned();
            response.action_kind = action.kind().as_str().to_owned();
            response.idempotency_key = action
                .idempotency_key()
                .map(|key| key.as_str().to_owned())
                .unwrap_or_default();
            let (label, record, conflict) = ack_outcome_parts(outcome);
            response.outcome = label.to_owned();
            response.delivery = record.map(attention_delivery_record_state);
            response.conflict = conflict;
        }
        IntegratorAttentionAcknowledged::Processed(outcome) => {
            response.acknowledgement = "processed".to_owned();
            if let Some(action) = outcome.record().and_then(|record| record.state().action()) {
                response.action_kind = action.kind().as_str().to_owned();
                response.idempotency_key = action
                    .idempotency_key()
                    .map(|key| key.as_str().to_owned())
                    .unwrap_or_default();
            }
            let (label, record, conflict) = processed_outcome_parts(outcome);
            response.outcome = label.to_owned();
            response.delivery = record.map(attention_delivery_record_state);
            response.conflict = conflict;
        }
        IntegratorAttentionAcknowledged::Failed(outcome) => {
            response.acknowledgement = "failed".to_owned();
            let (label, record) = match outcome {
                DeliveryFailureOutcome::Requeued(record) => ("requeued", Some(record)),
                DeliveryFailureOutcome::Exhausted(record) => ("exhausted", Some(record)),
                DeliveryFailureOutcome::LeaseNotOwned => ("lease_not_owned", None),
            };
            response.outcome = label.to_owned();
            response.delivery = record.map(attention_delivery_record_state);
        }
    }
    response
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

fn scope_from_proto(
    scope: Option<pb::IntegratorScopeState>,
) -> Result<IntegratorScope, DomainError> {
    let scope = scope.ok_or(DomainError::EmptyField {
        field: "integrator_scope",
    })?;
    match scope.kind.as_str() {
        "ceremony" => Ok(IntegratorScope::ceremony(CeremonyId::new(
            scope.ceremony_id,
        )?)),
        "system_execution" => Ok(IntegratorScope::system_execution(
            AgenticSystemExecutionId::new(scope.system_execution_id)?,
        )),
        _ => Err(DomainError::InvariantViolated {
            reason: "an integrator scope must be a ceremony or a system_execution",
        }),
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

fn acknowledgement_from_proto(
    acknowledgement: &str,
    action_kind: &str,
    idempotency_key: &str,
    note: &str,
    evidence: &str,
    failure_reason: &str,
) -> Result<IntegratorAcknowledgement, DomainError> {
    match acknowledgement {
        "intent" => {
            let action = action_from_proto(action_kind, idempotency_key)?;
            Ok(IntegratorAcknowledgement::Intent {
                note: DeliveryNote::new(if note.is_empty() {
                    format!("integrator intends {}", action.kind())
                } else {
                    note.to_owned()
                })?,
                evidence: optional(evidence.to_owned())
                    .map(EvidenceReference::new)
                    .transpose()?,
                action,
            })
        }
        "processed" => Ok(IntegratorAcknowledgement::Processed {
            action: action_from_proto(action_kind, idempotency_key)?,
        }),
        "failed" => Ok(IntegratorAcknowledgement::Failed {
            reason: DeliveryFailureReason::new(failure_reason)?,
        }),
        _ => Err(DomainError::InvariantViolated {
            reason: "an acknowledgement must be intent, processed or failed",
        }),
    }
}

fn action_from_proto(kind: &str, key: &str) -> Result<ProcessedActionRef, DomainError> {
    let kind: ProcessedActionKind =
        serde_json::from_value(serde_json::Value::String(kind.to_owned())).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "an action kind must be one of the seven the contract names",
            }
        })?;
    Ok(ProcessedActionRef::new(
        kind,
        optional(key.to_owned())
            .map(IdempotencyKey::new)
            .transpose()?,
    ))
}

fn delivery_state_from_proto(raw: &str) -> Result<HostDeliveryStateKind, DomainError> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "a delivery state must be one of the eight the contract names",
        }
    })
}

fn activation_from_proto(raw: &str) -> Result<HostActivationMode, DomainError> {
    match raw {
        "" | "none" => Ok(HostActivationMode::None),
        "command" => Ok(HostActivationMode::Command),
        _ => Err(DomainError::InvariantViolated {
            reason: "an activation mode must be none or command",
        }),
    }
}

/// Zero means "the server's own maximum", which is what the schema says.
fn page_limit(limit: u32) -> Result<HostDeliveryPageLimit, DomainError> {
    if limit == 0 {
        return Ok(HostDeliveryPageLimit::default());
    }
    HostDeliveryPageLimit::new(limit)
}

/// The fence is a transparent counter on the wire and in the domain,
/// so it crosses as the number it is rather than by counting up to it.
fn fence_from_proto(fence: u64) -> IntegratorFence {
    serde_json::from_value(serde_json::Value::from(fence)).unwrap_or(IntegratorFence::FIRST)
}

/// The one spelling of every enum on this surface: what serde writes,
/// which is what the schemas declare.
fn label(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_else(|_| at.to_string())
}
