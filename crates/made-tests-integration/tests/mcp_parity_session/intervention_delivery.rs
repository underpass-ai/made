use made_core::value_objects::{
    CeremonyId, ExecutionOperationId, StateIteration, StateVisit, StepId, StepIteration,
};
use serde_json::{json, Value};

use super::SESSION_ID;

/// A session of this part's own, so the roles it needs are declared
/// without disturbing the definitions other parts publish and diff.
const DELIVERY_SESSION_ID: &str = "parity-intervention-delivery-session";

const DELIVERY_CEREMONY: &str = r#"
version: "1.0"
name: "parity_intervention_delivery"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: noop
roles:
  - id: ENGINEER
    allowed_actions:
      - work
      - finish
      - respond_to_intervention
  - id: LEAD
    allowed_actions:
      - request_intervention
      - finish
"#;

/// Intervention delivery, driven on both backends.
///
/// The whole journey against a live claim: a question put to one exact
/// execution, pulled under a lease, acknowledged, answered and read
/// back. The lease id is minted per engine, so the acknowledgement
/// presents the ticket its own arm issued — comparing one scripted
/// literal against two different tickets would compare an
/// acknowledgement with a refusal and call the difference parity.
pub(super) fn script() -> Vec<(&'static str, Value)> {
    let operation_id = ExecutionOperationId::for_step(
        &CeremonyId::new(DELIVERY_SESSION_ID).unwrap(),
        &StepId::new("work").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
    )
    .to_string();
    vec![
        // What the earlier session's own items look like once the
        // delivery projection is asked about them.
        (
            "made_get_ceremony_intervention",
            json!({ "ceremony_id": SESSION_ID, "intervention_id": "what-happened" }),
        ),
        (
            "made_list_ceremony_interventions",
            json!({ "ceremony_id": SESSION_ID }),
        ),
        (
            "made_list_ceremony_interventions",
            json!({ "ceremony_id": SESSION_ID, "unresolved_only": true }),
        ),
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "definition_yaml": DELIVERY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "grpc-fixture-host",
                "idempotency_key": "parity-delivery-claim",
                "lease_ttl_ms": 60_000,
            }),
        ),
        // The claim fence is filled in by the scripted host from the
        // claim above, per arm.
        (
            "made_report_ceremony_agent_status",
            json!({ "status": {
                "ceremony_id": DELIVERY_SESSION_ID,
                "agent_execution_id": "parity-delivery-execution",
                "operation_id": operation_id,
                "claim_owner_id": "grpc-fixture-host",
                "logical_worker_id": "parity-delivery-worker",
                "host_agent_id": "grpc-fixture-host",
                "host_agent_incarnation": "parity-delivery-incarnation-1",
                "role_id": "ENGINEER",
                "step_id": "work",
                "attempt": 1,
                "execution_status": "running",
                "liveness": "fresh",
                "source": "host_report",
                "activity": "working",
                "task_summary": "Running the parity delivery claim.",
                "evidence_references": [],
                "observed_at": "2026-09-16T09:00:00Z",
                "report_sequence": 1,
                "idempotency_key": "parity-delivery-status-1",
            }}),
        ),
        (
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "intervention_id": "ask-the-live-agent",
                "role_id": "LEAD",
                "role_kind": "human",
                "kind": "opinion",
                "intent": "question",
                "target_agent_execution_id": "parity-delivery-execution",
                "target_incarnation": "parity-delivery-incarnation-1",
                "target_role_id": "ENGINEER",
                "message": "Is the work still on plan?",
            }),
        ),
        (
            "made_pull_ceremony_agent_interventions",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "agent_execution_id": "parity-delivery-execution",
                "incarnation": "parity-delivery-incarnation-1",
                "role_id": "ENGINEER",
            }),
        ),
        (
            "made_acknowledge_ceremony_agent_intervention",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "intervention_id": "ask-the-live-agent",
                "delivery_id": "$pulled:parity-delivery-execution",
                "lease_id": "$pulled",
                "agent_execution_id": "parity-delivery-execution",
                "incarnation": "parity-delivery-incarnation-1",
                "role_id": "ENGINEER",
                "observation_kind": "received",
                "note": "the parity agent has it",
                "observed_at": "2026-09-16T09:00:00Z",
            }),
        ),
        (
            "made_get_ceremony_intervention",
            json!({
                "ceremony_id": DELIVERY_SESSION_ID,
                "intervention_id": "ask-the-live-agent",
            }),
        ),
        (
            "made_list_ceremony_interventions",
            json!({ "ceremony_id": DELIVERY_SESSION_ID, "limit": 10 }),
        ),
    ]
}
