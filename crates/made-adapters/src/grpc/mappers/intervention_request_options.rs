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

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{FollowReplacement, HostDeliveryMode};
    use prost_types::{value::Kind, Struct, Value};

    fn pb_struct(fields: &[(&str, Kind)]) -> Struct {
        Struct {
            fields: fields
                .iter()
                .map(|(key, kind)| {
                    (
                        (*key).to_owned(),
                        Value {
                            kind: Some(kind.clone()),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_four_intents_are_read_and_anything_else_is_refused() {
        for (raw, expected) in [
            ("question", CeremonyInterventionIntent::Question),
            ("feedback", CeremonyInterventionIntent::Feedback),
            ("constraint", CeremonyInterventionIntent::Constraint),
            ("checkpoint", CeremonyInterventionIntent::Checkpoint),
            ("  question  ", CeremonyInterventionIntent::Question),
        ] {
            assert_eq!(intent_from_proto(raw).unwrap(), expected, "{raw}");
        }
        assert!(intent_from_proto("urgent").is_err());
        assert!(intent_from_proto("").is_err());
    }

    /// Every field omitted means the engine's own terms, not a policy
    /// of zeroes: a lease of no length excludes nobody, and an attempt
    /// limit of none would offer an item forever.
    #[test]
    fn an_empty_policy_is_the_engines_defaults_and_not_a_policy_of_zeroes() {
        let policy = delivery_policy_from_proto(&pb_struct(&[])).unwrap();
        let host = policy.host_policy();
        assert_eq!(host.mode(), HostDeliveryMode::PullLease);
        assert_eq!(host.lease_duration().get(), 60_000);
        assert_eq!(host.max_attempts().value(), 3);
        assert!(host.ack_timeout().is_none());
        assert_eq!(policy.follow_replacement(), FollowReplacement::Stay);
    }

    #[test]
    fn a_stated_policy_is_read_field_for_field() {
        let policy = delivery_policy_from_proto(&pb_struct(&[
            ("mode", Kind::StringValue("activation".to_owned())),
            ("lease_duration_ms", Kind::NumberValue(1_000.0)),
            ("ack_timeout_ms", Kind::NumberValue(5_000.0)),
            ("max_attempts", Kind::NumberValue(2.0)),
            ("follow_replacement", Kind::BoolValue(true)),
        ]))
        .unwrap();
        let host = policy.host_policy();
        assert_eq!(host.mode(), HostDeliveryMode::Activation);
        assert_eq!(host.lease_duration().get(), 1_000);
        assert_eq!(host.ack_timeout().unwrap().get(), 5_000);
        assert_eq!(host.max_attempts().value(), 2);
        assert_eq!(policy.follow_replacement(), FollowReplacement::Follow);
    }

    #[test]
    fn a_lease_outside_the_bounds_is_refused_rather_than_clamped() {
        for millis in [0.0, 86_400_000.0] {
            let refused = delivery_policy_from_proto(&pb_struct(&[(
                "lease_duration_ms",
                Kind::NumberValue(millis),
            )]));
            assert!(refused.is_err(), "{millis} was accepted");
        }
    }

    /// An unknown mode reads as the default rather than as activation:
    /// claiming a deployment can wake a host when it cannot is the one
    /// mistake this field can make.
    #[test]
    fn an_unrecognised_mode_falls_back_to_pulling() {
        let policy = delivery_policy_from_proto(&pb_struct(&[(
            "mode",
            Kind::StringValue("push".to_owned()),
        )]))
        .unwrap();
        assert_eq!(policy.host_policy().mode(), HostDeliveryMode::PullLease);
    }

    #[test]
    fn a_supervisor_needs_both_an_identity_and_a_name() {
        let supervisor = supervisor_from_proto(&pb_struct(&[
            ("principal_id", Kind::StringValue("alice".to_owned())),
            ("display", Kind::StringValue("Release manager".to_owned())),
        ]))
        .unwrap();
        assert_eq!(supervisor.principal_id().as_str(), "alice");
        assert_eq!(supervisor.display().as_str(), "Release manager");
        assert_eq!(
            supervisor.requesting_role().unwrap().as_str(),
            "supervisor:alice",
            "the asking seat is derived, so it cannot be a declared role"
        );

        assert!(supervisor_from_proto(&pb_struct(&[(
            "principal_id",
            Kind::StringValue("alice".to_owned())
        )]))
        .is_err());
        assert!(supervisor_from_proto(&pb_struct(&[(
            "display",
            Kind::StringValue("Release manager".to_owned())
        )]))
        .is_err());
    }
}
