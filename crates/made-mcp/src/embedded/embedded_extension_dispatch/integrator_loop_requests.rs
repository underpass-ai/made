//! What a host says, read into the five inputs of the loop.
//!
//! Apart from the dispatcher because reading a request is where the
//! refusals live — an unknown scope, an acknowledgement that names no
//! act, a fence that is not a number — and a dispatcher that also did
//! this would be a file nobody reads and everybody edits.

use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionInput, AwaitIntegratorAttentionInput,
    BindCeremonyIntegratorInput, IntegratorAcknowledgement, ListAttentionDeliveriesInput,
    DEFAULT_LEASE, DEFAULT_WAIT,
};
use made_core::ports::{BindReplacement, HostDeliveryPageLimit};
use made_core::value_objects::{
    AgenticSystemExecutionId, CeremonyId, DeliveryFailureReason, DeliveryNote, DurationMs,
    EvidenceReference, FollowReplacement, HostActivationMode, HostAddress, HostAgentIncarnation,
    HostDeliveryId, HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryStateKind, HostDestination,
    HostKind, IdempotencyKey, IntegratorBindingId, IntegratorFence, IntegratorScope,
    ProcessedActionKind, ProcessedActionRef, RoleId,
};
use serde_json::{Map, Value};
use time::OffsetDateTime;

use crate::protocol::ToolError;

pub(super) fn bind(object: &Map<String, Value>) -> Result<BindCeremonyIntegratorInput, ToolError> {
    Ok(BindCeremonyIntegratorInput {
        binding_id: IntegratorBindingId::new(required(object, "binding_id")?)?,
        scope: scope(object)?,
        role_id: RoleId::new(required(object, "role_id")?)?,
        destination: HostDestination::new(
            HostKind::new(required(object, "host_kind")?)?,
            HostAddress::new(required(object, "address")?)?,
            activation(object)?,
        ),
        incarnation: HostAgentIncarnation::new(required(object, "incarnation")?)?,
        replacement: if flag(object, "replace") {
            BindReplacement::Replace
        } else {
            BindReplacement::Refuse
        },
        follow: if flag(object, "follow_replacement") {
            FollowReplacement::Follow
        } else {
            FollowReplacement::Stay
        },
    })
}

pub(super) fn binding_scope(object: &Map<String, Value>) -> Result<IntegratorScope, ToolError> {
    scope(object)
}

pub(super) fn await_attention(
    object: &Map<String, Value>,
) -> Result<AwaitIntegratorAttentionInput, ToolError> {
    Ok(AwaitIntegratorAttentionInput {
        scope: scope(object)?,
        binding_id: IntegratorBindingId::new(required(object, "binding_id")?)?,
        incarnation: HostAgentIncarnation::new(required(object, "incarnation")?)?,
        fence: fence(object)?,
        limit: limit(object)?,
        // Absent is "say nothing about waiting", not "do not wait": a
        // caller that omitted the field still gets the short hold the
        // schema promises it.
        wait: object
            .get("wait_timeout_ms")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_WAIT, DurationMs::from_millis),
        lease_duration: object
            .get("lease_duration_ms")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_LEASE, DurationMs::from_millis),
    })
}

pub(super) fn acknowledge(
    object: &Map<String, Value>,
) -> Result<AcknowledgeIntegratorAttentionInput, ToolError> {
    let incarnation = HostAgentIncarnation::new(required(object, "incarnation")?)?;
    let delivery_id = HostDeliveryId::new(required(object, "delivery_id")?)?;
    Ok(AcknowledgeIntegratorAttentionInput {
        binding_id: IntegratorBindingId::new(required(object, "binding_id")?)?,
        incarnation: incarnation.clone(),
        fence: fence(object)?,
        delivery_id: delivery_id.clone(),
        lease: HostDeliveryLease::new(
            delivery_id,
            HostDeliveryLeaseId::new(required(object, "lease_id")?)?,
            incarnation,
            // The ledger compares the lease identifier, not this
            // instant: a host repeating what it was handed does not get
            // to extend its own hold by saying so.
            OffsetDateTime::now_utc(),
        ),
        outcome: acknowledgement(object)?,
    })
}

pub(super) fn deliveries(
    object: &Map<String, Value>,
) -> Result<ListAttentionDeliveriesInput, ToolError> {
    Ok(ListAttentionDeliveriesInput {
        binding_id: optional(object, "binding_id")
            .map(IntegratorBindingId::new)
            .transpose()?,
        ceremony_id: optional(object, "ceremony_id")
            .map(CeremonyId::new)
            .transpose()?,
        state: optional(object, "state")
            .map(|raw| {
                serde_json::from_value::<HostDeliveryStateKind>(Value::String(raw)).map_err(|_| {
                    ToolError::invalid_request(
                        "state must be one of the eight the catalogue declares",
                    )
                })
            })
            .transpose()?,
        limit: limit(object)?,
        cursor: optional(object, "cursor")
            .map(HostDeliveryId::new)
            .transpose()?,
    })
}

fn acknowledgement(object: &Map<String, Value>) -> Result<IntegratorAcknowledgement, ToolError> {
    match required(object, "acknowledgement")? {
        "intent" => {
            let action = action(object)?;
            Ok(IntegratorAcknowledgement::Intent {
                note: DeliveryNote::new(
                    optional(object, "note")
                        .unwrap_or_else(|| format!("integrator intends {}", action.kind())),
                )?,
                evidence: optional(object, "evidence")
                    .map(EvidenceReference::new)
                    .transpose()?,
                action,
            })
        }
        "processed" => Ok(IntegratorAcknowledgement::Processed {
            action: action(object)?,
        }),
        "failed" => Ok(IntegratorAcknowledgement::Failed {
            reason: DeliveryFailureReason::new(required(object, "failure_reason")?)?,
        }),
        _ => Err(ToolError::invalid_request(
            "acknowledgement must be intent, processed or failed",
        )),
    }
}

fn action(object: &Map<String, Value>) -> Result<ProcessedActionRef, ToolError> {
    let kind: ProcessedActionKind =
        serde_json::from_value(Value::String(required(object, "action_kind")?.to_owned()))
            .map_err(|_| {
                ToolError::invalid_request(
                    "action_kind must be one of the seven the catalogue declares",
                )
            })?;
    Ok(ProcessedActionRef::new(
        kind,
        optional(object, "idempotency_key")
            .map(IdempotencyKey::new)
            .transpose()?,
    ))
}

fn scope(object: &Map<String, Value>) -> Result<IntegratorScope, ToolError> {
    let scope = object
        .get("scope")
        .and_then(Value::as_object)
        .ok_or_else(|| ToolError::invalid_request("field `scope` is required"))?;
    match required(scope, "kind")? {
        "ceremony" => Ok(IntegratorScope::ceremony(CeremonyId::new(required(
            scope,
            "ceremony_id",
        )?)?)),
        "system_execution" => Ok(IntegratorScope::system_execution(
            AgenticSystemExecutionId::new(required(scope, "system_execution_id")?)?,
        )),
        _ => Err(ToolError::invalid_request(
            "scope.kind must be ceremony or system_execution",
        )),
    }
}

fn activation(object: &Map<String, Value>) -> Result<HostActivationMode, ToolError> {
    match optional(object, "activation").as_deref() {
        None | Some("none") => Ok(HostActivationMode::None),
        Some("command") => Ok(HostActivationMode::Command),
        Some(_) => Err(ToolError::invalid_request(
            "activation must be none or command",
        )),
    }
}

/// Required rather than defaulted: the first fence is zero, and a
/// caller that omitted it would be presenting the first generation of
/// a binding it may have been displaced from.
fn fence(object: &Map<String, Value>) -> Result<IntegratorFence, ToolError> {
    let raw = object
        .get("fence")
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::invalid_request("field `fence` is required"))?;
    serde_json::from_value(Value::from(raw))
        .map_err(|_| ToolError::invalid_request("fence must be a whole number"))
}

/// Absent means "the server's own maximum", which is what the schema
/// says.
fn limit(object: &Map<String, Value>) -> Result<HostDeliveryPageLimit, ToolError> {
    match object.get("limit").and_then(Value::as_u64) {
        None => Ok(HostDeliveryPageLimit::default()),
        Some(value) => Ok(HostDeliveryPageLimit::new(
            u32::try_from(value)
                .map_err(|_| ToolError::invalid_request("limit is out of range"))?,
        )?),
    }
}

fn flag(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool).unwrap_or_default()
}

fn required<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, ToolError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToolError::invalid_request(format!("missing required string `{key}`")))
}

fn optional(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
