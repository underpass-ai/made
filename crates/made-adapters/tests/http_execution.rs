#![cfg(feature = "_http")]

mod execution_support;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use execution_support::{request, scratch};
use made_adapters::connectors::{HttpExecutionConnector, HttpOperationResponse};
use made_core::ports::{
    CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
};
use made_core::value_objects::{
    ExecutionConnectorId, ExecutionRequestDigest, StepOutput, StepResult,
};
use made_core::DomainError;
use wiremock::matchers::path_regex;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn response(request: &made_core::ports::CeremonyExecutionRequest) -> serde_json::Value {
    serde_json::to_value(HttpOperationResponse {
        operation_id: request.intent().operation().operation_id().clone(),
        request_digest: request.intent().operation().request_digest().clone(),
        producer_claim_fence: request.intent().claim_fence().clone(),
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        observed_at: time::OffsetDateTime::UNIX_EPOCH,
    })
    .unwrap()
}

async fn server(
    durable: Arc<Mutex<Option<serde_json::Value>>>,
    puts: Arc<AtomicUsize>,
) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(path_regex(r"^/operations/[0-9a-f]{64}$"))
        .respond_with(move |request: &Request| {
            if request.method.as_str() == "GET" {
                return durable.lock().unwrap().clone().map_or_else(
                    || ResponseTemplate::new(404),
                    |body| ResponseTemplate::new(200).set_body_json(body),
                );
            }
            if request.method.as_str() != "PUT"
                || request.headers.get("idempotency-key").is_none()
                || request.headers.get("x-request-digest").is_none()
            {
                return ResponseTemplate::new(400);
            }
            puts.fetch_add(1, Ordering::SeqCst);
            let submitted: serde_json::Value = request.body_json().unwrap();
            let result = serde_json::json!({
                "operation_id": submitted["operation_id"],
                "request_digest": submitted["request_digest"],
                "producer_claim_fence": submitted["producer_claim_fence"],
                "result": StepResult::completed(StepOutput::empty()).unwrap(),
                "observed_at": "1970-01-01T00:00:00Z",
            });
            *durable.lock().unwrap() = Some(result);
            // The accepted effect is durable, but its direct response is lost.
            ResponseTemplate::new(503)
        })
        .mount(&server)
        .await;
    server
}

fn connector(server: &MockServer, operation_root: &std::path::Path) -> HttpExecutionConnector {
    HttpExecutionConnector::new(
        ExecutionConnectorId::new("http").unwrap(),
        &server.uri(),
        operation_root,
        Duration::from_secs(2),
    )
    .unwrap()
}

fn observed(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Observed(_))
}

#[tokio::test]
async fn response_loss_is_recovered_by_stable_operation_lookup() {
    let durable = Arc::new(Mutex::new(None));
    let puts = Arc::new(AtomicUsize::new(0));
    let server = server(Arc::clone(&durable), Arc::clone(&puts)).await;
    let root = scratch();
    let connector = connector(&server, root.path());
    let request = request("http", serde_json::json!({"http": {"action": "accept"}}));

    assert!(observed(
        &connector.execute_or_recover(request.clone()).await.unwrap()
    ));
    assert_eq!(puts.load(Ordering::SeqCst), 1);
    assert!(observed(
        &connector.recover_intent(request.intent()).await.unwrap()
    ));
    assert_eq!(puts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn duplicate_workers_share_one_durable_admission() {
    let durable = Arc::new(Mutex::new(None));
    let puts = Arc::new(AtomicUsize::new(0));
    let server = server(Arc::clone(&durable), Arc::clone(&puts)).await;
    let root = scratch();
    let connector = Arc::new(connector(&server, root.path()));
    let request = request("http", serde_json::json!({"http": {"action": "once"}}));

    let (first, second) = tokio::join!(
        connector.execute_or_recover(request.clone()),
        connector.execute_or_recover(request.clone()),
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(observed(
        &connector.recover_intent(request.intent()).await.unwrap()
    ));
    assert_eq!(puts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn remote_identity_or_digest_mismatch_is_a_conflict() {
    let root = scratch();
    let request = request("http", serde_json::json!({"http": {"action": "read"}}));
    let mut mismatched = response(&request);
    mismatched["request_digest"] =
        serde_json::to_value(ExecutionRequestDigest::new("0".repeat(64)).unwrap()).unwrap();
    let durable = Arc::new(Mutex::new(Some(mismatched)));
    let server = server(durable, Arc::new(AtomicUsize::new(0))).await;

    assert!(matches!(
        connector(&server, root.path())
            .recover_intent(request.intent())
            .await,
        Err(DomainError::Conflict { .. })
    ));
}
