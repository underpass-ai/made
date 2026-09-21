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
        wait: millis(object, "wait_timeout_ms").unwrap_or(DEFAULT_WAIT),
        lease_duration: millis(object, "lease_duration_ms").unwrap_or(DEFAULT_LEASE),
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

/// Absent or zero means "the server's own maximum", which is what the
/// contract says and what the gRPC surface does with the same request.
fn limit(object: &Map<String, Value>) -> Result<HostDeliveryPageLimit, ToolError> {
    match stated(object, "limit") {
        None => Ok(HostDeliveryPageLimit::default()),
        Some(value) => Ok(HostDeliveryPageLimit::new(
            u32::try_from(value)
                .map_err(|_| ToolError::invalid_request("limit is out of range"))?,
        )?),
    }
}

/// A duration a caller actually named, in milliseconds.
fn millis(object: &Map<String, Value>, key: &str) -> Option<DurationMs> {
    stated(object, key).map(DurationMs::from_millis)
}

/// A number the caller stated, where zero is not a statement.
///
/// The contract reads a zero optional number as "take the engine's
/// default", and the proto surface has no way to tell an unset field
/// from a zero one, so it must. Read the same way here rather than
/// literally: taken literally, `lease_duration_ms: 0` is a lease that
/// has already expired and `limit: 0` is a refusal, so the same request
/// would work on one backend and misbehave on the other — which is
/// exactly the drift the parity gate exists to catch, and it cannot see
/// it, because the two arms would both be answering the request they
/// were given.
fn stated(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn await_arguments(extra: &Value) -> Map<String, Value> {
        let mut object = json!({
            "scope": { "kind": "ceremony", "ceremony_id": "c-1" },
            "binding_id": "b-1",
            "incarnation": "run-1",
            "fence": 0,
        });
        let target = object.as_object_mut().expect("the base is an object");
        for (key, value) in extra.as_object().expect("the extra is an object") {
            target.insert(key.clone(), value.clone());
        }
        target.clone()
    }

    /// A zero lease is not a lease: it has expired before the host has
    /// read the answer. The contract says zero takes the engine's own,
    /// and the gRPC surface has to read it that way because a proto
    /// scalar cannot tell unset from zero. This surface can, and still
    /// must not, or the same request behaves differently per backend.
    #[test]
    fn a_zero_lease_takes_the_default_rather_than_expiring_on_arrival() {
        let input = await_attention(&await_arguments(&json!({ "lease_duration_ms": 0 })))
            .expect("the request is well formed");
        assert_eq!(input.lease_duration, DEFAULT_LEASE);

        let stated = await_attention(&await_arguments(&json!({ "lease_duration_ms": 5_000 })))
            .expect("the request is well formed");
        assert_eq!(stated.lease_duration, DurationMs::from_millis(5_000));
    }

    #[test]
    fn a_zero_wait_takes_the_default_rather_than_not_waiting() {
        let input = await_attention(&await_arguments(&json!({ "wait_timeout_ms": 0 })))
            .expect("the request is well formed");
        assert_eq!(input.wait, DEFAULT_WAIT);

        let stated = await_attention(&await_arguments(&json!({ "wait_timeout_ms": 2_500 })))
            .expect("the request is well formed");
        assert_eq!(stated.wait, DurationMs::from_millis(2_500));
    }

    /// Taken literally a zero page is refused, and the gRPC surface
    /// answers a hundred: a caller sending the same arguments would get
    /// an error from one backend and a page from the other.
    #[test]
    fn a_zero_page_takes_the_maximum_rather_than_being_refused() {
        let input = await_attention(&await_arguments(&json!({ "limit": 0 })))
            .expect("the request is well formed");
        assert_eq!(input.limit, HostDeliveryPageLimit::default());

        let page = json!({ "limit": 0 });
        let page =
            deliveries(page.as_object().expect("an object")).expect("the read is well formed");
        assert_eq!(page.limit, HostDeliveryPageLimit::default());
    }
}
