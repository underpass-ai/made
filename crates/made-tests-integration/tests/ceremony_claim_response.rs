//! The direct RPC response must describe its own accepted claim under replacement.
use std::sync::Arc;
use std::time::Duration;

use made_app::services::SessionStream;
use made_core::entities::{AuditChain, AuditRecord};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StepId, StreamVersion};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    ClaimCeremonyStepRequest, ClaimCeremonyStepResponse, CompleteCeremonyStepRequest,
    StartCeremonyRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use tokio::time::timeout;
use tonic::{Code, Request};

#[path = "ceremony_claim_response/controlled_clock.rs"]
mod controlled_clock;
#[path = "ceremony_claim_response/paused_claim_store.rs"]
mod paused_claim_store;
use controlled_clock::ControlledClock;
use paused_claim_store::PausedClaimStore;

const DEFINITION: &str = r#"
version: "1.0"
name: claim_response
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
    handler: host_callback
roles:
  - id: WORKER
    allowed_actions: [work, finish]
retry_policies:
  default:
    max_attempts: 3
    backoff_seconds: 0
"#;

fn claim(owner: &str, trace: &str) -> Request<ClaimCeremonyStepRequest> {
    let mut request = Request::new(ClaimCeremonyStepRequest {
        ceremony_id: "claim-response".to_owned(),
        step_id: "work".to_owned(),
        actor_kind: "agent".to_owned(),
        lease_owner_id: owner.to_owned(),
        idempotency_key: owner.to_owned(),
        lease_ttl_ms: 1000,
        budget_reservation: None,
        execution_profile: None,
    });
    request
        .metadata_mut()
        .insert("traceparent", trace.parse().unwrap());
    request
}

fn complete(fence: String) -> CompleteCeremonyStepRequest {
    CompleteCeremonyStepRequest {
        ceremony_id: "claim-response".to_owned(),
        step_id: "work".to_owned(),
        actor_kind: "agent".to_owned(),
        status: "completed".to_owned(),
        claim_fence: fence,
        ..Default::default()
    }
}

#[tokio::test]
async fn claim_response_keeps_accepted_snapshot_and_audit_identity_after_replacement() {
    let store = Arc::new(PausedClaimStore::default());
    let clock = Arc::new(ControlledClock::default());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_ceremony_store(store.clone())
            .with_clock(clock.clone()),
    )
    .await;
    let mut a = MadeServiceClient::new(fixture.channel.clone());
    let mut b = a.clone();
    a.start_ceremony(StartCeremonyRequest {
        ceremony_id: "claim-response".to_owned(),
        actor_id: "operator".to_owned(),
        actor_kind: "service".to_owned(),
        definition_yaml: DEFINITION.to_owned(),
        context: None,
    })
    .await
    .unwrap();
    let first = tokio::spawn(async move {
        a.claim_ceremony_step(claim(
            "worker-a",
            "00-11111111111111111111111111111111-1111111111111111-01",
        ))
        .await
        .unwrap()
        .into_inner()
    });
    timeout(Duration::from_secs(5), store.committed.notified())
        .await
        .unwrap();
    clock.expire_first_claim();
    let second = b
        .claim_ceremony_step(claim(
            "worker-b",
            "00-22222222222222222222222222222222-2222222222222222-01",
        ))
        .await
        .unwrap()
        .into_inner();
    store.resume.notify_one();
    let first = timeout(Duration::from_secs(5), first)
        .await
        .unwrap()
        .unwrap();
    let id = CeremonyId::new("claim-response").unwrap();
    let records = store
        .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_claim_responses(&first, &second, &records);
    let refused = b
        .complete_ceremony_step(complete(first.claim_fence))
        .await
        .unwrap_err();
    assert_eq!(refused.code(), Code::FailedPrecondition);
    assert_eq!(
        store
            .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
            .await
            .unwrap(),
        records
    );
    let completed = b
        .complete_ceremony_step(complete(second.claim_fence.clone()))
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(completed.steps[0].attempt, 2);
    assert_eq!(completed.steps[0].status, "completed");
    let retried = b
        .complete_ceremony_step(complete(second.claim_fence))
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(retried, completed);
    let final_records = store
        .read(&id, StreamVersion::EMPTY, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(final_records.len(), records.len() + 1);
    assert!(AuditChain::verify(&final_records).is_intact());
}

fn assert_claim_responses(
    first: &ClaimCeremonyStepResponse,
    second: &ClaimCeremonyStepResponse,
    records: &[AuditRecord],
) {
    let a_state = first.instance.as_ref().unwrap();
    let b_state = second.instance.as_ref().unwrap();
    assert_eq!(a_state.steps[0].attempt, 1);
    assert_eq!(b_state.steps[0].attempt, 2);
    assert_eq!(a_state.trace_id, "11111111111111111111111111111111");
    assert_eq!(b_state.trace_id, "22222222222222222222222222222222");
    let a_record = records
        .iter()
        .find(|record| record.trace_id() == Some(a_state.trace_id.as_str()))
        .unwrap();
    let b_record = records.last().unwrap();
    assert_eq!(
        a_state.correlation_id,
        a_record.correlation_id().unwrap().as_str()
    );
    assert_eq!(
        a_state.causation_id,
        a_record.causation_id().unwrap().as_str()
    );
    assert_eq!(
        b_state.causation_id,
        b_record.causation_id().unwrap().as_str()
    );
    assert_ne!(a_state.causation_id, b_state.causation_id);
    assert_ne!(first.claim_fence, second.claim_fence);
    let a_end = records.partition_point(|record| record.sequence() <= a_record.sequence());
    let accepted_a = SessionStream::fold_records(&records[..a_end])
        .unwrap()
        .instance;
    assert_eq!(
        accepted_a
            .step_claim_fence(&StepId::new("work").unwrap())
            .unwrap()
            .as_str(),
        first.claim_fence
    );
    let accepted_b = SessionStream::fold_records(records).unwrap().instance;
    assert_eq!(
        accepted_b
            .step_claim_fence(&StepId::new("work").unwrap())
            .unwrap()
            .as_str(),
        second.claim_fence
    );
}
