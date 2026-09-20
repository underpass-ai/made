//! The optional halves of a request for an intervention.
//!
//! Its own file because each of these reads a shape the caller chose
//! not to fill in, and a mapper that gets these wrong does not fail —
//! it quietly asks for something other than what was asked for.

use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorId, CeremonyInterventionIntent, DeliveryAttemptLimit, DurationMs, FollowReplacement,
    HostDeliveryMode, HostDeliveryPolicy, InterventionDeliveryPolicy, SupervisorDisplayName,
    SupervisorPrincipal,
};

use super::attributes::attributes_from_struct;

pub(super) fn intent_from_proto(raw: &str) -> Result<CeremonyInterventionIntent, DomainError> {
    serde_json::from_value(serde_json::Value::String(raw.trim().to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "intervention intent must be question, feedback, constraint or checkpoint",
        }
    })
}

/// Reads the policy fields it understands and leaves the rest at the
/// engine's own defaults, which is what the schema promises.
pub(super) fn delivery_policy_from_proto(
    delivery: &prost_types::Struct,
) -> Result<InterventionDeliveryPolicy, DomainError> {
    let fields = attributes_json(delivery);
    let mode = match fields.get("mode").and_then(serde_json::Value::as_str) {
        Some("activation") => HostDeliveryMode::Activation,
        _ => HostDeliveryMode::PullLease,
    };
    let lease = fields
        .get("lease_duration_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(60_000);
    let ack_timeout = fields
        .get("ack_timeout_ms")
        .and_then(serde_json::Value::as_u64)
        .map(DurationMs::from_millis);
    let attempts = fields
        .get("max_attempts")
        .and_then(serde_json::Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(u32::MAX))
        .map(DeliveryAttemptLimit::new)
        .transpose()?
        .unwrap_or_default();
    let follow = if fields
        .get("follow_replacement")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        FollowReplacement::Follow
    } else {
        FollowReplacement::Stay
    };
    Ok(InterventionDeliveryPolicy::new(HostDeliveryPolicy::new(
        mode,
        DurationMs::from_millis(lease),
        ack_timeout,
        attempts,
        follow,
    )?))
}

pub(super) fn supervisor_from_proto(
    supervisor: &prost_types::Struct,
) -> Result<SupervisorPrincipal, DomainError> {
    let fields = attributes_json(supervisor);
    let principal_id = fields
        .get("principal_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(DomainError::EmptyField {
            field: "supervisor.principal_id",
        })?;
    let display = fields
        .get("display")
        .and_then(serde_json::Value::as_str)
        .ok_or(DomainError::EmptyField {
            field: "supervisor.display",
        })?;
    Ok(SupervisorPrincipal::new(
        AuditActorId::new(principal_id),
        SupervisorDisplayName::new(display)?,
    ))
}

/// A protobuf struct as plain JSON, so the fields above read the same
/// way the embedded backend reads them.
fn attributes_json(value: &prost_types::Struct) -> serde_json::Map<String, serde_json::Value> {
    attributes_from_struct(Some(value.clone()))
        .ok()
        .map(|attributes| attributes.as_map().clone().into_iter().collect())
        .unwrap_or_default()
}
