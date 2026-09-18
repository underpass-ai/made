//! The delegated-host protocol: proto → application.
//!
//! Claiming and completing are one move split in two around work the
//! engine never sees. Between them the host runs the step with its
//! own agents, tools or hands, which can take minutes, so the claim
//! defaults to a lease long enough to survive that — the same
//! five-minute default the embedded tool has always used, rather than
//! the thirty seconds an in-engine step takes.
//!
//! Like their in-engine sibling, these conversions ask the definition
//! which seat runs a step and take only the kind that filled it from
//! the caller. An adapter that answered the first question itself
//! would be a second copy of the engine's authorization rules.

use made_app::usecases::{CompleteCeremonyStepInput, StartCeremonyStepInput};
use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, StepErrorMessage, StepId, StepOutput,
    StepResult, StepStatus,
};
use made_proto::v1 as pb;
use uuid::Uuid;

use super::actor_kind::actor_kind_from_proto;
use super::attributes::attributes_from_struct;

const DEFAULT_LEASE_OWNER_ID: &str = "grpc-claim-ceremony-step";

/// The engine's own number, read from the input that carries it.
const DEFAULT_LEASE_TTL_MS: u64 = StartCeremonyStepInput::DEFAULT_LEASE_TTL_MS;

/// The statuses a host may report. Deliberately not every
/// [`StepStatus`]: `pending` and `in_progress` describe a step nobody
/// has finished with, and reporting one as a result would be a host
/// telling the engine it has nothing to tell it.
const OBSERVABLE_STATUSES: [StepStatus; 4] = [
    StepStatus::Completed,
    StepStatus::Failed,
    StepStatus::WaitingForHuman,
    StepStatus::Cancelled,
];

/// Take the lease for a step the host is about to execute itself.
///
/// The session is named, never the definition: what an instance runs
/// is settled when it starts, so there is nothing here a caller could
/// use to point a running session at a definition of their choosing.
pub fn claim_ceremony_step_input_from_proto(
    request: pb::ClaimCeremonyStepRequest,
    definition: &CeremonyDefinition,
    instance: &CeremonyInstance,
) -> Result<StartCeremonyStepInput, DomainError> {
    let step_id = StepId::new(request.step_id)?;
    let role_id = definition.role_id_for_step(&step_id)?;
    let role_kind = actor_kind_from_proto(&request.actor_kind, "actor_kind")?;
    let lease_owner_id = if request.lease_owner_id.trim().is_empty() {
        LeaseOwnerId::new(DEFAULT_LEASE_OWNER_ID)?
    } else {
        LeaseOwnerId::new(request.lease_owner_id)?
    };
    // An absent key still produces one, so nothing downstream has to
    // cope with its absence. It buys no replay protection, which is
    // what asking for none means.
    let idempotency_key = if request.idempotency_key.trim().is_empty() {
        IdempotencyKey::new(format!("grpc-claim-{}", Uuid::new_v4()))?
    } else {
        IdempotencyKey::new(request.idempotency_key)?
    };
    let lease_ttl = if request.lease_ttl_ms == 0 {
        DurationMs::from_millis(DEFAULT_LEASE_TTL_MS)
    } else {
        DurationMs::from_millis(request.lease_ttl_ms)
    };

    Ok(StartCeremonyStepInput::new(
        instance.id().clone(),
        role_id,
        role_kind,
        step_id,
        lease_owner_id,
        idempotency_key,
        lease_ttl,
    )
    .with_automatic_role_resolution())
}

/// Record what the host saw when it ran the step.
///
/// Neither the definition nor the loaded session is needed here: the
/// use case resolves both from the instance it loads, and the seat
/// that finished the step is the definition's to say.
pub fn complete_ceremony_step_input_from_proto(
    request: pb::CompleteCeremonyStepRequest,
) -> Result<CompleteCeremonyStepInput, DomainError> {
    let status = step_status_from_proto(&request.status)?;
    let output = StepOutput::new(attributes_from_struct(request.output)?);
    let error = if request.error.trim().is_empty() {
        None
    } else {
        Some(StepErrorMessage::new(request.error)?)
    };

    Ok(CompleteCeremonyStepInput::new(
        CeremonyId::new(request.ceremony_id)?,
        StepId::new(request.step_id)?,
        // Refuses a failure with no reason and a success carrying one.
        StepResult::new(status, output, error)?,
        actor_kind_from_proto(&request.actor_kind, "actor_kind")?,
    ))
}

/// Read off the status's own label rather than a second table of
/// spellings, so the wire and the domain cannot drift apart.
fn step_status_from_proto(raw: &str) -> Result<StepStatus, DomainError> {
    OBSERVABLE_STATUSES
        .into_iter()
        .find(|status| status.as_label() == raw)
        .ok_or(DomainError::InvalidCharacters { field: "status" })
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::AuditActorKind;
    use time::OffsetDateTime;

    use super::*;
    use crate::yaml::CeremonyDefinitionYaml;

    const EDITORIAL_MEETING_CEREMONY: &str =
        include_str!("../../../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

    fn definition() -> CeremonyDefinition {
        CeremonyDefinitionYaml::parse_str(EDITORIAL_MEETING_CEREMONY).unwrap()
    }

    fn started_instance(definition: &CeremonyDefinition) -> CeremonyInstance {
        CeremonyInstance::start(
            CeremonyId::new("ceremony-delegation-1").unwrap(),
            definition,
            serde_json::from_value(serde_json::json!({"meeting_brief":"fixture meeting"})).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .expect("required ceremony inputs")
    }

    fn claim_request(step_id: &str) -> pb::ClaimCeremonyStepRequest {
        pb::ClaimCeremonyStepRequest {
            ceremony_id: "ceremony-delegation-1".to_owned(),
            step_id: step_id.to_owned(),
            actor_kind: "agent".to_owned(),
            lease_owner_id: String::new(),
            idempotency_key: String::new(),
            lease_ttl_ms: 0,
        }
    }

    fn complete_request(status: &str) -> pb::CompleteCeremonyStepRequest {
        pb::CompleteCeremonyStepRequest {
            ceremony_id: "ceremony-delegation-1".to_owned(),
            step_id: "open_room".to_owned(),
            actor_kind: "human".to_owned(),
            status: status.to_owned(),
            output: None,
            error: String::new(),
        }
    }

    #[test]
    fn a_claim_leases_as_the_role_the_definition_authorizes() {
        let definition = definition();
        let instance = started_instance(&definition);

        let input = claim_ceremony_step_input_from_proto(
            claim_request("open_room"),
            &definition,
            &instance,
        )
        .unwrap();

        assert_eq!(input.role_id().as_str(), "FACILITATOR");
        assert_eq!(input.lease_owner_id().as_str(), DEFAULT_LEASE_OWNER_ID);
        assert!(input.idempotency_key().as_str().starts_with("grpc-claim-"));
    }

    /// The lease has to outlive work the engine is not doing.
    #[test]
    fn an_unstated_lease_lasts_as_long_as_host_work_plausibly_does() {
        let definition = definition();
        let instance = started_instance(&definition);

        let input = claim_ceremony_step_input_from_proto(
            claim_request("open_room"),
            &definition,
            &instance,
        )
        .unwrap();

        assert_eq!(input.lease_ttl().get(), DEFAULT_LEASE_TTL_MS);
    }

    #[test]
    fn an_explicit_lease_and_key_are_carried_through_untouched() {
        let definition = definition();
        let instance = started_instance(&definition);

        let input = claim_ceremony_step_input_from_proto(
            pb::ClaimCeremonyStepRequest {
                lease_owner_id: "host-runner-3".to_owned(),
                idempotency_key: "claim-42".to_owned(),
                lease_ttl_ms: 5_000,
                ..claim_request("open_room")
            },
            &definition,
            &instance,
        )
        .unwrap();

        assert_eq!(input.lease_owner_id().as_str(), "host-runner-3");
        assert_eq!(input.idempotency_key().as_str(), "claim-42");
        assert_eq!(input.lease_ttl().get(), 5_000);
    }

    #[test]
    fn a_step_no_role_is_authorized_for_is_refused_before_it_is_claimed() {
        let definition = definition();
        let instance = started_instance(&definition);

        let error = claim_ceremony_step_input_from_proto(
            claim_request("not_a_step"),
            &definition,
            &instance,
        )
        .unwrap_err();

        assert!(matches!(error, DomainError::InvariantViolated { .. }));
    }

    /// Naming another ceremony must not move this session onto it.
    #[test]
    fn the_session_claimed_is_the_one_loaded_not_the_one_named() {
        let definition = definition();
        let instance = started_instance(&definition);

        let input = claim_ceremony_step_input_from_proto(
            pb::ClaimCeremonyStepRequest {
                ceremony_id: "some-other-ceremony".to_owned(),
                ..claim_request("open_room")
            },
            &definition,
            &instance,
        )
        .unwrap();

        assert_eq!(input.instance_id(), instance.id());
    }

    #[test]
    fn every_observable_status_the_schema_offers_is_understood() {
        for status in OBSERVABLE_STATUSES {
            let request = if status == StepStatus::Failed {
                pb::CompleteCeremonyStepRequest {
                    error: "the tool exited 1".to_owned(),
                    ..complete_request(status.as_label())
                }
            } else {
                complete_request(status.as_label())
            };

            let input = complete_ceremony_step_input_from_proto(request)
                .unwrap_or_else(|error| panic!("{} was refused: {error}", status.as_label()));
            assert_eq!(input.result().status(), status);
        }
    }

    #[test]
    fn a_status_nobody_can_observe_is_refused() {
        for raw in ["", "pending", "in_progress", "COMPLETED", "done"] {
            assert!(
                complete_ceremony_step_input_from_proto(complete_request(raw)).is_err(),
                "{raw:?} was accepted as an observable result"
            );
        }
    }

    #[test]
    fn a_failure_with_no_reason_and_a_success_with_one_are_both_refused() {
        assert!(complete_ceremony_step_input_from_proto(complete_request("failed")).is_err());
        assert!(
            complete_ceremony_step_input_from_proto(pb::CompleteCeremonyStepRequest {
                error: "but it worked".to_owned(),
                ..complete_request("completed")
            })
            .is_err()
        );
    }

    #[test]
    fn the_host_output_reaches_the_result() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "artifact".to_owned(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("s3://minutes".into())),
            },
        );

        let input = complete_ceremony_step_input_from_proto(pb::CompleteCeremonyStepRequest {
            output: Some(prost_types::Struct { fields }),
            ..complete_request("completed")
        })
        .unwrap();

        assert_eq!(
            input.result().output().attributes().get("artifact"),
            Some(&serde_json::json!("s3://minutes"))
        );
        assert_eq!(input.actor_kind(), AuditActorKind::Human);
    }
}
