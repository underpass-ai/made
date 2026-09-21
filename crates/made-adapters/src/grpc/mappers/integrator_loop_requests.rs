//! What a host claims about itself, read into the five inputs.
//!
//! Apart from the projection next door by direction rather than by
//! size: this half refuses — an unknown scope, an acknowledgement that
//! names no act, a page size out of range — and the other half has
//! nothing to refuse, because it renders what the engine already
//! holds. Splitting them that way keeps every refusal in one file.

use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionInput, AwaitIntegratorAttentionInput,
    BindCeremonyIntegratorInput, IntegratorAcknowledgement, ListAttentionDeliveriesInput,
    DEFAULT_LEASE, DEFAULT_WAIT,
};
use made_core::error::DomainError;
use made_core::ports::{BindReplacement, HostDeliveryPageLimit};
use made_core::value_objects::{
    AgenticSystemExecutionId, CeremonyId, DeliveryFailureReason, DeliveryNote, DurationMs,
    EvidenceReference, FollowReplacement, HostActivationMode, HostAddress, HostAgentIncarnation,
    HostDeliveryId, HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryStateKind, HostDestination,
    HostKind, IdempotencyKey, IntegratorBindingId, IntegratorFence, IntegratorScope,
    ProcessedActionKind, ProcessedActionRef, RoleId,
};
use made_proto::v1 as pb;
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
            FollowReplacement::Follow
        } else {
            FollowReplacement::Stay
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

/// An empty wire string is a field nobody filled in.
fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
