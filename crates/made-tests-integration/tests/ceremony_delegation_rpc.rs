//! The delegated-host protocol over gRPC.
//!
//! A host that runs the step itself claims it, does the work where
//! the engine cannot see it, and then reports what happened. What is
//! tested here is what the engine promises across that gap: one
//! claim per step however many times the call is repeated, a result
//! that reaches the session as the host described it, and a refusal
//! for a step the session is not on.

use made_core::value_objects::{
    CeremonyId, ExecutionOperationId, StateIteration, StateVisit, StepId, StepIteration,
};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    CeremonyAgentStatus, CeremonyInstanceState, ClaimCeremonyStepRequest,
    CompleteCeremonyStepRequest, GetCeremonyAgentRequest, GetCeremonyInstanceRequest,
    ListCeremonyAgentsRequest, ReportCeremonyAgentStatusRequest, StartCeremonyRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use prost_types::Timestamp;
use prost_types::{value::Kind, Struct, Value};
use tonic::transport::Channel;
use tonic::Code;

const EDITORIAL_MEETING_CEREMONY: &str =
    include_str!("../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

#[tokio::test]
async fn delegated_renewal_replays_and_finishes_after_the_initial_ttl() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "delegated-renewal";
    start(&mut client, ceremony_id).await;
    let mut request = claim(ceremony_id, "open_room", "accepted-host-operation");
    request.lease_ttl_ms = 500;
    let claimed = client
        .claim_ceremony_step(request)
        .await
        .unwrap()
        .into_inner();
    let heartbeat = made_proto::v1::RenewCeremonyStepLeaseRequest {
        ceremony_id: ceremony_id.into(),
        step_id: "open_room".into(),
        claim_fence: claimed.claim_fence.clone(),
        lease_owner_id: "grpc-fixture-host".into(),
        renewal_id: "host-heartbeat-1".into(),
        lease_ttl_ms: 5000,
    };
    let accepted = client
        .renew_ceremony_step_lease(heartbeat.clone())
        .await
        .unwrap()
        .into_inner();
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let replay = client
        .renew_ceremony_step_lease(heartbeat.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted, replay);
    let mut changed = heartbeat.clone();
    changed.lease_ttl_ms += 1;
    assert!(client.renew_ceremony_step_lease(changed).await.is_err());
    let state = read(&mut client, ceremony_id).await;
    assert_eq!(
        step(&state, "open_room").effective_lease_expires_at,
        accepted.effective_lease_expires_at
    );
    let result = client
        .complete_ceremony_step(CompleteCeremonyStepRequest {
            ceremony_id: ceremony_id.into(),
            step_id: "open_room".into(),
            actor_kind: "agent".into(),
            status: "completed".into(),
            claim_fence: claimed.claim_fence,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(step(&result, "open_room").status, "completed");
}

async fn start(
    client: &mut MadeServiceClient<Channel>,
    ceremony_id: &str,
) -> CeremonyInstanceState {
    client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: ceremony_id.to_owned(),
            actor_id: "operator-1".to_owned(),
            actor_kind: "service".to_owned(),
            definition_yaml: EDITORIAL_MEETING_CEREMONY.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "meeting_brief".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue(
                            "fixture meeting".to_owned(),
                        )),
                    },
                )]
                .into_iter()
                .collect(),
            }),
        })
        .await
        .expect("StartCeremony should succeed")
        .into_inner()
        .instance
        .expect("a started ceremony must come back")
}

async fn read(client: &mut MadeServiceClient<Channel>, ceremony_id: &str) -> CeremonyInstanceState {
    client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .expect("GetCeremonyInstance should succeed")
        .into_inner()
        .instance
        .expect("a started ceremony must be readable")
}

fn claim(ceremony_id: &str, step_id: &str, key: &str) -> ClaimCeremonyStepRequest {
    ClaimCeremonyStepRequest {
        ceremony_id: ceremony_id.to_owned(),
        step_id: step_id.to_owned(),
        actor_kind: "agent".to_owned(),
        lease_owner_id: "grpc-fixture-host".to_owned(),
        idempotency_key: key.to_owned(),
        lease_ttl_ms: 60_000,
        budget_reservation: None,
        execution_profile: None,
    }
}

fn reported_agent_status(
    ceremony_id: &str,
    operation: &ExecutionOperationId,
    claim_fence: String,
) -> CeremonyAgentStatus {
    CeremonyAgentStatus {
        ceremony_id: ceremony_id.into(),
        agent_execution_id: "worker-execution".into(),
        operation_id: operation.to_string(),
        claim_owner_id: "grpc-fixture-host".into(),
        logical_worker_id: "worker".into(),
        host_agent_id: "grpc-fixture-host".into(),
        host_agent_incarnation: "inc-1".into(),
        previous_host_agent_id: None,
        previous_host_agent_incarnation: None,
        role_id: "facilitator".into(),
        step_id: "open_room".into(),
        attempt: 1,
        execution_status: "running".into(),
        liveness: "fresh".into(),
        source: "host_report".into(),
        requested_model: None,
        requested_reasoning_effort: None,
        actual_model: None,
        actual_reasoning_effort: None,
        activity: "working".into(),
        blocker: None,
        dependency: None,
        task_summary: "bounded".into(),
        evidence_references: vec![],
        usage_kind: Some("unavailable".into()),
        usage_value: None,
        observed_at: Some(Timestamp {
            seconds: 0,
            nanos: 0,
        }),
        report_sequence: 1,
        idempotency_key: "agent-status-report".into(),
        claim_fence,
    }
}

#[tokio::test]
async fn public_agent_status_follows_a_real_claim_and_is_readable() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "agent-status-public";
    start(&mut client, ceremony_id).await;
    let claimed = client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "agent-status-claim"))
        .await
        .unwrap()
        .into_inner();
    let operation = ExecutionOperationId::for_step(
        &CeremonyId::new(ceremony_id).unwrap(),
        &StepId::new("open_room").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
    );
    let status = reported_agent_status(ceremony_id, &operation, claimed.claim_fence);
    client
        .report_ceremony_agent_status(ReportCeremonyAgentStatusRequest {
            status: Some(status.clone()),
        })
        .await
        .unwrap();
    let observed = client
        .get_ceremony_agent(GetCeremonyAgentRequest {
            ceremony_id: ceremony_id.into(),
            agent_execution_id: "worker-execution".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .agent
        .unwrap();
    assert_eq!(observed.execution_status, "running");
    assert_eq!(observed.liveness, "stale");
    let original_fence = status.claim_fence.clone();
    let mut foreign = status;
    foreign.claim_owner_id = "foreign-host".into();
    foreign.host_agent_id = "foreign-host".into();
    foreign.claim_fence = "0".repeat(64);
    foreign.report_sequence = 2;
    foreign.idempotency_key = "foreign-status-report".into();
    assert!(client
        .report_ceremony_agent_status(ReportCeremonyAgentStatusRequest {
            status: Some(foreign)
        })
        .await
        .is_err());
    let retained = client
        .get_ceremony_agent(GetCeremonyAgentRequest {
            ceremony_id: ceremony_id.into(),
            agent_execution_id: "worker-execution".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .agent
        .unwrap();
    assert_eq!(retained.claim_owner_id, "grpc-fixture-host");
    assert_eq!(retained.claim_fence, original_fence);
    assert_eq!(retained.report_sequence, 1);
    assert_eq!(
        client
            .list_ceremony_agents(ListCeremonyAgentsRequest {
                ceremony_id: ceremony_id.into(),
                execution_status: None,
                cursor: None,
                limit: 10
            })
            .await
            .unwrap()
            .into_inner()
            .agents
            .len(),
        1
    );
}

fn step<'a>(
    instance: &'a CeremonyInstanceState,
    step_id: &str,
) -> &'a made_proto::v1::CeremonyStepState {
    instance
        .steps
        .iter()
        .find(|step| step.step_id == step_id)
        .unwrap_or_else(|| panic!("step {step_id} missing from the session"))
}

#[tokio::test]
async fn a_host_claims_a_step_does_the_work_and_reports_what_happened() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-delegation";

    let started = start(&mut client, ceremony_id).await;
    assert_eq!(started.next_step_id, "open_room");

    // Claiming answers with the session, like every other move, and
    // that session already shows the step taken on. A claim a caller
    // has to follow with a read to see is a claim it cannot act on.
    let claim = client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "integration-delegation-1"))
        .await
        .expect("ClaimCeremonyStep should succeed")
        .into_inner();
    let claim_fence = claim.claim_fence;
    let claimed = claim
        .instance
        .expect("a claimed step must come back with its session");
    assert_eq!(step(&claimed, "open_room").status, "in_progress");
    assert_eq!(step(&claimed, "open_room").attempt, 1);
    // Claiming performs no work: the guard that waits on this step
    // being done is not satisfied by having taken it on.
    assert!(claimed
        .transitions
        .iter()
        .all(|transition| !transition.enabled));
    assert_eq!(read(&mut client, ceremony_id).await, claimed);

    // …the host does the real work here, outside the engine…

    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "minutes_url".to_owned(),
        Value {
            kind: Some(Kind::StringValue("s3://minutes/open-room".to_owned())),
        },
    );
    let completed = client
        .complete_ceremony_step(CompleteCeremonyStepRequest {
            claim_fence: claim_fence.clone(),
            ceremony_id: ceremony_id.to_owned(),
            step_id: "open_room".to_owned(),
            actor_kind: "human".to_owned(),
            status: "completed".to_owned(),
            output: Some(Struct { fields }),
            error: String::new(),
        })
        .await
        .expect("CompleteCeremonyStep should succeed")
        .into_inner()
        .instance
        .expect("a completed step must come back with its session");

    let finished = step(&completed, "open_room");
    assert_eq!(finished.status, "completed", "{}", finished.error);
    // The host's own output is what the session carries: the engine
    // wrote down what it was told and produced nothing of its own.
    assert_eq!(
        finished
            .output
            .as_ref()
            .and_then(|output| output.fields.get("minutes_url").and_then(|value| {
                match &value.kind {
                    Some(Kind::StringValue(text)) => Some(text.clone()),
                    _ => None,
                }
            })),
        Some("s3://minutes/open-room".to_owned())
    );
    // And the session moved on exactly as it would have if the engine
    // had run the step itself.
    let enabled = completed
        .transitions
        .iter()
        .find(|transition| transition.trigger == "context_shared")
        .expect("the way out of OPENING should be listed");
    assert!(enabled.enabled);
    assert_eq!(read(&mut client, ceremony_id).await, completed);
}

/// Repeating a claim with the same key must not claim the step twice.
///
/// This is the call a host makes when it never saw the first answer,
/// which is the case the key exists for. The retry is refused and the
/// session is left with the one claim it already had — not a second
/// attempt, and not a lease handed to a runner that believes it is
/// the only one holding it.
#[tokio::test]
async fn replaying_a_claim_does_not_claim_the_step_twice() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-delegation-replay";

    start(&mut client, ceremony_id).await;
    client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "integration-replay"))
        .await
        .expect("the first claim should succeed");

    let status = client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "integration-replay"))
        .await
        .expect_err("a replayed claim must not take the step a second time");
    assert_ne!(status.code(), Code::Unknown);

    let after = read(&mut client, ceremony_id).await;
    assert_eq!(step(&after, "open_room").status, "in_progress");
    assert_eq!(step(&after, "open_room").attempt, 1);
}

/// A second host asking for a step that is already someone's.
#[tokio::test]
async fn a_second_runner_is_refused_while_the_lease_is_held() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-delegation-contended";

    start(&mut client, ceremony_id).await;
    client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "integration-first"))
        .await
        .expect("the first claim should succeed");

    let status = client
        .claim_ceremony_step(ClaimCeremonyStepRequest {
            lease_owner_id: "another-host".to_owned(),
            ..claim(ceremony_id, "open_room", "integration-second")
        })
        .await
        .expect_err("a step under an active lease must not be claimed again");
    assert_ne!(status.code(), Code::Unknown);
}

#[tokio::test]
async fn a_claim_on_a_step_outside_the_current_state_is_refused() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-delegation-out-of-order";

    start(&mut client, ceremony_id).await;

    let status = client
        // Belongs to SYNTHESIZING; the session is in OPENING.
        .claim_ceremony_step(claim(
            ceremony_id,
            "decision_summary",
            "integration-out-of-order",
        ))
        .await
        .expect_err("a step outside the current state must not be claimed");
    assert_ne!(status.code(), Code::Unknown);

    let after = read(&mut client, ceremony_id).await;
    assert_eq!(after.current_state, "OPENING");
    assert!(after
        .steps
        .iter()
        .all(|step| step.status != "in_progress" && step.status != "completed"));
}

/// A failure is a result too, and it has to say why.
#[tokio::test]
async fn a_reported_failure_reaches_the_session_with_its_reason() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-delegation-failure";

    start(&mut client, ceremony_id).await;
    let claim_fence = client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "integration-failure"))
        .await
        .expect("the claim should succeed")
        .into_inner()
        .claim_fence;

    let reasonless = client
        .complete_ceremony_step(CompleteCeremonyStepRequest {
            claim_fence: claim_fence.clone(),
            ceremony_id: ceremony_id.to_owned(),
            step_id: "open_room".to_owned(),
            actor_kind: "agent".to_owned(),
            status: "failed".to_owned(),
            output: None,
            error: String::new(),
        })
        .await
        .expect_err("a failure that says nothing must be refused");
    assert_eq!(reasonless.code(), Code::InvalidArgument);

    let failed = client
        .complete_ceremony_step(CompleteCeremonyStepRequest {
            claim_fence: claim_fence.clone(),
            ceremony_id: ceremony_id.to_owned(),
            step_id: "open_room".to_owned(),
            actor_kind: "agent".to_owned(),
            status: "failed".to_owned(),
            output: None,
            error: "the host's transcription tool exited 1".to_owned(),
        })
        .await
        .expect("a failure with a reason should be recorded")
        .into_inner()
        .instance
        .expect("a completed step must come back with its session");

    let record = step(&failed, "open_room");
    assert_eq!(record.status, "failed");
    assert_eq!(record.error, "the host's transcription tool exited 1");
    // A failure does not move the session on.
    assert_eq!(failed.current_state, "OPENING");
    assert!(failed
        .transitions
        .iter()
        .all(|transition| !transition.enabled));
}

#[tokio::test]
async fn completion_requires_the_returned_fence_and_refusals_append_nothing() {
    use made_adapters::memory::InMemoryCeremonyEventStore;
    use made_core::ports::CeremonyEventStorePort;
    use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};
    use std::sync::Arc;
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture = GrpcFixture::start_over(store.clone()).await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = "integration-required-fence";
    start(&mut client, ceremony_id).await;
    let accepted = client
        .claim_ceremony_step(claim(ceremony_id, "open_room", "required-fence"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.claim_fence.len(), 64);
    let id = CeremonyId::new(ceremony_id).unwrap();
    let before = store
        .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    for (fence, code) in [
        (String::new(), Code::InvalidArgument),
        ("malformed".to_owned(), Code::InvalidArgument),
        ("0".repeat(64), Code::FailedPrecondition),
    ] {
        let error = client
            .complete_ceremony_step(CompleteCeremonyStepRequest {
                ceremony_id: ceremony_id.to_owned(),
                step_id: "open_room".to_owned(),
                actor_kind: "agent".to_owned(),
                status: "completed".to_owned(),
                claim_fence: fence,
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(
            store
                .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
                .await
                .unwrap(),
            before
        );
    }
    client
        .complete_ceremony_step(CompleteCeremonyStepRequest {
            ceremony_id: ceremony_id.to_owned(),
            step_id: "open_room".to_owned(),
            actor_kind: "agent".to_owned(),
            status: "completed".to_owned(),
            claim_fence: accepted.claim_fence,
            ..Default::default()
        })
        .await
        .unwrap();
}
