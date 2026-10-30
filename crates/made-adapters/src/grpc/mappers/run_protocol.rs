//! Protocol execution: proto ↔ application.

use made_app::usecases::{RunProtocolInput, RunProtocolOutput};
use made_core::error::DomainError;
use made_core::value_objects::{DurationMs, LeaseOwnerId, ProtocolContext, ProtocolId, StepStatus};
use made_proto::v1 as pb;
use uuid::Uuid;

use crate::mermaid::ProtocolConversationDiagram;
use crate::yaml::ProtocolDefinitionYaml;

use super::actor_kind::actor_kind_from_proto;
use super::attributes::attributes_from_struct;

const DEFAULT_LEASE_OWNER_ID: &str = "grpc-run-protocol";

/// The engine's own number, read from the input that carries it
/// rather than written again here: a direct gRPC client that omits
/// the field gets the same lease as every other caller.
const DEFAULT_LEASE_TTL_MS: u64 = RunProtocolInput::DEFAULT_LEASE_TTL_MS;

pub fn run_protocol_input_from_proto(
    request: pb::RunProtocolRequest,
) -> Result<RunProtocolInput, DomainError> {
    let id = if request.protocol_id.trim().is_empty() {
        ProtocolId::new(Uuid::new_v4().to_string())?
    } else {
        ProtocolId::new(request.protocol_id)?
    };
    let definition = ProtocolDefinitionYaml::parse_str(&request.definition_yaml)?;
    let context = ProtocolContext::new(attributes_from_struct(request.context)?);
    let lease_owner_id = if request.lease_owner_id.trim().is_empty() {
        LeaseOwnerId::new(DEFAULT_LEASE_OWNER_ID)?
    } else {
        LeaseOwnerId::new(request.lease_owner_id)?
    };
    let lease_ttl = if request.lease_ttl_ms == 0 {
        DurationMs::from_millis(DEFAULT_LEASE_TTL_MS)
    } else {
        DurationMs::from_millis(request.lease_ttl_ms)
    };

    Ok(RunProtocolInput::new(
        id,
        definition,
        context,
        lease_owner_id,
        lease_ttl,
        request.actor_id,
        actor_kind_from_proto(&request.actor_kind, "actor_kind")?,
    ))
}

pub fn run_protocol_response_from(output: &RunProtocolOutput) -> pb::RunProtocolResponse {
    let instance = output.instance();
    let definition = output.definition();
    pb::RunProtocolResponse {
        protocol_id: instance.id().as_str().to_owned(),
        definition_name: definition.name().as_str().to_owned(),
        definition_version: definition.version().as_str().to_owned(),
        final_state: instance.current_state().as_str().to_owned(),
        completed: instance.is_completed(definition),
        steps: output
            .step_traces()
            .iter()
            .map(|trace| pb::ProtocolStepExecution {
                state_id: trace.state_id().as_str().to_owned(),
                step_id: trace.step_id().as_str().to_owned(),
                role_id: trace.role_id().as_str().to_owned(),
                status: step_status_to_proto(trace.status()).to_owned(),
                attempt: trace.attempt().get(),
                // Surface the deliberation winner's content so the response
                // carries the meeting's record, not just step statuses.
                output: trace
                    .output()
                    .attributes()
                    .get(crate::protocol::WINNER_CONTENT_KEY)
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                iteration: trace.iteration().get(),
                state_visit: trace.state_visit().get(),
                state_iteration: trace.state_iteration().get(),
            })
            .collect(),
        mermaid_sequence: ProtocolConversationDiagram::render(definition),
    }
}

fn step_status_to_proto(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "PENDING",
        StepStatus::InProgress => "IN_PROGRESS",
        StepStatus::Completed => "COMPLETED",
        StepStatus::Failed => "FAILED",
        StepStatus::WaitingForHuman => "WAITING_FOR_HUMAN",
        StepStatus::Cancelled => "CANCELLED",
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{ProtocolName, StepOutput, StepResult};

    use super::*;
    use made_app::usecases::ProtocolStepTrace;
    use made_core::entities::ProtocolInstance;
    use made_core::value_objects::{RoleId, StateId, StepAttempt, StepId};
    use time::macros::datetime;

    const PROTOCOL: &str = r#"
version: "1.0"
name: "rpc_protocol"
states:
  - id: STARTED
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: STARTED
    to: DONE
    trigger: finish
    guards:
      - work_completed
steps:
  - id: work
    state: STARTED
    handler: noop_step
guards:
  work_completed:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: RUNNER
    allowed_actions:
      - work
      - finish
"#;

    #[test]
    fn request_maps_yaml_to_run_protocol_input() {
        let input = run_protocol_input_from_proto(pb::RunProtocolRequest {
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            protocol_id: "protocol-rpc-1".to_owned(),
            definition_yaml: PROTOCOL.to_owned(),
            context: None,
            lease_owner_id: String::new(),
            lease_ttl_ms: 0,
        })
        .unwrap();

        assert_eq!(input.id().as_str(), "protocol-rpc-1");
        assert_eq!(
            input.definition().name(),
            &ProtocolName::new("rpc_protocol").unwrap()
        );
        assert_eq!(input.lease_owner_id().as_str(), DEFAULT_LEASE_OWNER_ID);
        assert_eq!(input.lease_ttl().get(), DEFAULT_LEASE_TTL_MS);
    }

    #[test]
    fn response_projects_step_trace_and_diagram() {
        let definition = ProtocolDefinitionYaml::parse_str(PROTOCOL).unwrap();
        let mut instance = ProtocolInstance::start(
            ProtocolId::new("protocol-rpc-1").unwrap(),
            &definition,
            ProtocolContext::empty(),
            datetime!(2026-06-06 12:00:00 UTC),
        )
        .expect("required protocol inputs");
        let step_id = StepId::new("work").unwrap();
        instance
            .start_step(
                &definition,
                &step_id,
                made_core::value_objects::StepLease::new(
                    LeaseOwnerId::new("runner").unwrap(),
                    made_core::value_objects::IdempotencyKey::new("key-1").unwrap(),
                    datetime!(2026-06-06 12:00:00 UTC),
                    datetime!(2026-06-06 12:01:00 UTC),
                )
                .unwrap(),
                datetime!(2026-06-06 12:00:00 UTC),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id,
                instance.step_claim_fence(&step_id).unwrap(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                datetime!(2026-06-06 12:00:00 UTC),
            )
            .unwrap();
        instance
            .apply_transition(
                &definition,
                &made_core::value_objects::TransitionTrigger::new("finish").unwrap(),
                datetime!(2026-06-06 12:00:00 UTC),
            )
            .unwrap();
        let output = RunProtocolOutput::new(
            definition,
            instance,
            vec![ProtocolStepTrace::new(
                StateId::new("STARTED").unwrap(),
                StepId::new("work").unwrap(),
                RoleId::new("RUNNER").unwrap(),
                StepAttempt::FIRST,
                StepStatus::Completed,
                StepOutput::new(
                    made_core::value_objects::Attributes::new(std::collections::BTreeMap::from([
                        (
                            crate::protocol::WINNER_CONTENT_KEY.to_owned(),
                            serde_json::json!("the agreed build plan"),
                        ),
                    ]))
                    .unwrap(),
                ),
            )],
        );

        let response = run_protocol_response_from(&output);

        assert!(response.completed);
        assert_eq!(response.final_state, "DONE");
        assert_eq!(response.steps.len(), 1);
        assert_eq!(response.steps[0].status, "COMPLETED");
        assert_eq!(response.steps[0].iteration, 1);
        assert_eq!(response.steps[0].attempt, 1);
        assert_eq!(response.steps[0].output, "the agreed build plan");
        assert!(response.mermaid_sequence.contains("sequenceDiagram"));
    }
}
