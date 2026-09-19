//! Wall-clock acceptance beyond the production 60-second admission TTL.
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use made_adapters::memory::InMemoryCeremonyEventStore;
use made_core::entities::AuditChain;
use made_core::ports::{
    CeremonyEventStorePort, CeremonyStepHandlerPort, CeremonyStepHandlerRequest,
};
use made_core::value_objects::{
    AuditEventType, CeremonyEventPageLimit, CeremonyId, StepOutput, StepResult, StreamVersion,
};
use made_core::DomainError;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{RunCeremonyRequest, RunCeremonyStepRequest, StartCeremonyRequest};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};

const DEFINITION: &str = r#"
version: "1.0"
name: real_time_admission
states:
  - {id: WORK, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: WORK, to: DONE, trigger: finish}
steps:
  - {id: work, state: WORK, handler: slow_fixture}
roles:
  - {id: WORKER, allowed_actions: [work, finish]}
"#;

struct SlowFixture;
#[async_trait]
impl CeremonyStepHandlerPort for SlowFixture {
    async fn execute(&self, _: CeremonyStepHandlerRequest) -> Result<StepResult, DomainError> {
        tokio::time::sleep(Duration::from_secs(65)).await;
        StepResult::completed(StepOutput::empty())
    }
}

#[tokio::test]
#[ignore = "65-second wall-clock acceptance; run explicitly on the release candidate"]
async fn real_rpc_run_and_run_step_outlive_the_default_admission_ttl() {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_ceremony_store(store.clone())
            .with_step_handler(Arc::new(SlowFixture)),
    )
    .await;
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", fixture.addr))
        .unwrap()
        .timeout(Duration::from_secs(90))
        .connect()
        .await
        .unwrap();
    let mut run_client = MadeServiceClient::new(channel);
    let mut step_client = run_client.clone();
    step_client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: "long-step".to_owned(),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
            definition_yaml: DEFINITION.to_owned(),
            context: None,
        })
        .await
        .unwrap();
    let started = Instant::now();
    let (whole, step) = tokio::join!(
        run_client.run_ceremony(RunCeremonyRequest {
            ceremony_id: "long-run".to_owned(),
            actor_id: "operator".to_owned(),
            actor_kind: "service".to_owned(),
            definition_yaml: DEFINITION.to_owned(),
            context: None,
            lease_owner_id: "run-worker".to_owned(),
            lease_ttl_ms: 180_000,
        }),
        step_client.run_ceremony_step(RunCeremonyStepRequest {
            ceremony_id: "long-step".to_owned(),
            step_id: "work".to_owned(),
            actor_kind: "agent".to_owned(),
            lease_owner_id: "step-worker".to_owned(),
            idempotency_key: "long-step".to_owned(),
            lease_ttl_ms: 180_000,
        }),
    );
    let whole = whole.unwrap().into_inner();
    let step = step.unwrap().into_inner();
    assert!(whole.completed);
    assert!(step.instance.is_some());
    assert!(started.elapsed() >= Duration::from_secs(65));
    for id in ["long-run", "long-step"] {
        let records = store
            .read(
                &CeremonyId::new(id).unwrap(),
                StreamVersion::EMPTY,
                CeremonyEventPageLimit::DEFAULT,
            )
            .await
            .unwrap();
        assert!(AuditChain::verify(&records).is_intact());
        let claim = records
            .iter()
            .find(|r| r.event_type() == AuditEventType::StepStarted)
            .unwrap();
        let result = records
            .iter()
            .find(|r| r.event_type() == AuditEventType::StepCompleted)
            .unwrap();
        let original = claim.authorization_evidence().unwrap();
        let renewed = result.authorization_evidence().unwrap();
        assert!(result.occurred_at() > original.valid_until());
        assert!(result.occurred_at() >= renewed.admitted_at());
        assert!(renewed.is_live_at(result.occurred_at()));
        assert_ne!(original.decision_id(), renewed.decision_id());
    }
    println!(
        "protected RPC wall-clock acceptance passed in {:.3}s",
        started.elapsed().as_secs_f64()
    );
}
