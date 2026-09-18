//! Aggregation execution through the deployed and embedded editions.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::RunCeremonyInput;
use made_core::entities::{AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, LeaseOwnerId, StepId,
    StepOutput, StepResult, StepStatus,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::{ParityClock, PARITY_INSTANT};
use serde_json::{json, Value};

const SESSION: &str = "aggregation-e2e";
const SIBLINGS: [&str; 3] = ["alpha", "beta", "gamma"];

#[derive(Debug)]
struct RecordingAggregationHandler {
    votes: [&'static str; 3],
    requests: Mutex<Vec<CeremonyStepHandlerRequest>>,
}

impl RecordingAggregationHandler {
    fn majority() -> Arc<Self> {
        Arc::new(Self {
            votes: ["ship", "ship", "hold"],
            requests: Mutex::new(Vec::new()),
        })
    }

    fn split() -> Arc<Self> {
        Arc::new(Self {
            votes: ["a", "b", "c"],
            requests: Mutex::new(Vec::new()),
        })
    }

    fn requests(&self) -> Vec<CeremonyStepHandlerRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn vote_for(&self, step_id: &StepId) -> &str {
        let index = SIBLINGS
            .iter()
            .position(|candidate| *candidate == step_id.as_str())
            .expect("the handler only votes for declared siblings");
        self.votes[index]
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for RecordingAggregationHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let output = if request.step_id().as_str() == "aggregate" {
            let inputs = request
                .transcript()
                .contributions()
                .iter()
                .map(|contribution| contribution.step_id().as_str())
                .collect::<Vec<_>>();
            StepOutput::new(Attributes::new(BTreeMap::from([
                ("summary".to_owned(), json!("synthesized")),
                ("inputs".to_owned(), json!(inputs)),
                ("winner_content".to_owned(), json!("synthesized")),
            ]))?)
        } else {
            StepOutput::new(Attributes::new(BTreeMap::from([(
                "choice".to_owned(),
                json!(self.vote_for(request.step_id())),
            )]))?)
        };
        self.requests.lock().unwrap().push(request);
        StepResult::completed(output)
    }
}

fn ceremony(strategy: &str) -> String {
    let aggregate = if strategy == "vote" {
        "aggregate: {strategy: vote, output_field: choice}"
    } else {
        "aggregate: {strategy: synthesize}"
    };
    format!(
        r#"version: "1.0"
name: aggregation_e2e
states:
  - {{id: REVIEW, initial: true, execution: concurrent}}
  - {{id: DECIDE}}
  - {{id: DONE, terminal: true}}
transitions:
  - {{from: REVIEW, to: DECIDE, trigger: reviews_done, guards: [all_reviews_done]}}
  - {{from: DECIDE, to: DONE, trigger: finish, guards: [aggregate_done]}}
steps:
  - {{id: alpha, state: REVIEW, handler: host_callback}}
  - {{id: beta, state: REVIEW, handler: host_callback}}
  - {{id: gamma, state: REVIEW, handler: host_callback}}
  - id: aggregate
    state: DECIDE
    handler: host_callback
    config: {{see_prior: true}}
    {aggregate}
guards:
  all_reviews_done: {{type: automated, check: "steps_completed:3"}}
  aggregate_done: {{type: automated, check: "step_status:aggregate:COMPLETED"}}
roles:
  - {{id: ALPHA, allowed_actions: [alpha, reviews_done]}}
  - {{id: BETA, allowed_actions: [beta]}}
  - {{id: GAMMA, allowed_actions: [gamma]}}
  - {{id: DECIDER, allowed_actions: [aggregate, finish]}}
max_parallel: 3
"#
    )
}

fn scratch() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn durable_engine(path: &Path, handler: Arc<dyn CeremonyStepHandlerPort>) -> EmbeddedMade {
    let store = Arc::new(SqliteCeremonyStore::open(path).unwrap());
    EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_definition_publications(store)
        .with_clock(ParityClock::shared())
        .with_step_handler(handler)
        .build()
}

async fn call(server: &MadeMcpServer, id: u64, tool: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": arguments,
            "_meta": {
                "traceparent": format!("00-{id:032x}-{id:016x}-01")
            }
        }
    });
    let line = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

fn structured(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "{answer:#}");
    assert_ne!(answer["result"]["isError"], true, "{answer:#}");
    &answer["result"]["structuredContent"]
}

fn run_arguments(yaml: &str) -> Value {
    json!({
        "ceremony_id": SESSION,
        "definition_yaml": yaml,
        "actor_id": "aggregation-test",
        "actor_kind": "service",
        "lease_owner_id": "aggregation-host",
        "lease_ttl_ms": 60_000
    })
}

fn aggregate_output(instance: &CeremonyInstance) -> Value {
    serde_json::to_value(
        instance
            .step_record(&StepId::new("aggregate").unwrap())
            .unwrap()
            .output()
            .attributes(),
    )
    .unwrap()
}

fn aggregate_output_from_mcp(instance: &Value) -> Value {
    instance["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["step_id"] == "aggregate")
        .unwrap()["output"]
        .clone()
}

fn canonical_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_json).collect()),
        Value::Object(fields) => {
            let mut entries = fields.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonical_json(value)))
                    .collect(),
            )
        }
        scalar => scalar,
    }
}

fn sort_json(values: &mut [Value]) {
    values.sort_by_key(|value| serde_json::to_string(value).unwrap());
}

fn semantic_history(answer: &Value) -> Vec<Value> {
    let mut events = structured(answer)["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| canonical_json(record["event"].clone()))
        .collect::<Vec<_>>();
    sort_json(&mut events);
    events
}

fn semantic_record_history(records: &[AuditRecord]) -> Vec<Value> {
    let mut events = records
        .iter()
        .filter_map(AuditRecord::event)
        .map(|event| canonical_json(serde_json::to_value(event).unwrap()))
        .collect::<Vec<_>>();
    sort_json(&mut events);
    events
}

fn assert_aggregate_finished(answer: &Value, ending: &str) {
    let aggregate_events = structured(answer)["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["event"]["step_id"] == "aggregate")
        .map(|record| record["event_type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(aggregate_events, ["step_started", ending]);
}

fn assert_vote_handler_calls(handler: &RecordingAggregationHandler) {
    let mut called = handler
        .requests()
        .iter()
        .map(|request| request.step_id().as_str().to_owned())
        .collect::<Vec<_>>();
    called.sort();
    assert_eq!(called, SIBLINGS);
}

fn assert_synthesis_handler_calls(handler: &RecordingAggregationHandler) {
    let requests = handler.requests();
    assert_eq!(requests.len(), 4);
    let request = requests
        .iter()
        .find(|request| request.step_id().as_str() == "aggregate")
        .unwrap();
    assert_eq!(
        request
            .transcript()
            .contributions()
            .iter()
            .map(|contribution| contribution.step_id().as_str())
            .collect::<Vec<_>>(),
        SIBLINGS
    );
    assert!(request
        .transcript()
        .contributions()
        .iter()
        .all(|contribution| contribution.output().attributes().get("choice").is_some()));
}

async fn run_facade(
    path: &Path,
    yaml: &str,
    handler: Arc<RecordingAggregationHandler>,
) -> (CeremonyInstance, Vec<Value>) {
    let definition = CeremonyDefinitionYaml::parse_str(yaml).unwrap();
    let engine = durable_engine(path, handler);
    let output = engine
        .run(RunCeremonyInput::new(
            CeremonyId::new(SESSION).unwrap(),
            definition,
            CeremonyContext::empty(),
            LeaseOwnerId::new("aggregation-host").unwrap(),
            DurationMs::from_millis(60_000),
            "aggregation-test",
            AuditActorKind::Service,
        ))
        .await
        .unwrap()
        .instance()
        .clone();
    let records = engine
        .audit_records(&CeremonyId::new(SESSION).unwrap())
        .await
        .unwrap();
    (output, semantic_record_history(&records))
}

async fn execute_parity(strategy: &str) -> (Vec<Arc<RecordingAggregationHandler>>, Value) {
    let yaml = ceremony(strategy);
    let remote_handler = RecordingAggregationHandler::majority();
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_clock(ParityClock::shared())
            .with_step_handler(remote_handler.clone()),
    )
    .await;
    let remote = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    ));

    let directory = scratch();
    let embedded_path = directory.path().join("embedded.sqlite3");
    let embedded_handler = RecordingAggregationHandler::majority();
    let embedded_engine = durable_engine(&embedded_path, embedded_handler.clone());
    let embedded = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(embedded_engine));

    let remote_run = call(&remote, 1, "made_run_ceremony", run_arguments(&yaml)).await;
    let embedded_run = call(&embedded, 1, "made_run_ceremony", run_arguments(&yaml)).await;
    assert_eq!(remote_run, embedded_run);
    assert!(structured(&remote_run)["completed"].as_bool().unwrap());

    let read = json!({"ceremony_id": SESSION});
    let remote_instance = call(&remote, 2, "made_get_ceremony_instance", read.clone()).await;
    let embedded_instance = call(&embedded, 2, "made_get_ceremony_instance", read.clone()).await;
    assert_eq!(remote_instance, embedded_instance);
    let remote_events = call(&remote, 3, "made_read_ceremony_events", read.clone()).await;
    let embedded_events = call(&embedded, 3, "made_read_ceremony_events", read).await;
    assert_eq!(
        semantic_history(&remote_events),
        semantic_history(&embedded_events)
    );
    assert_aggregate_finished(&remote_events, "step_completed");
    assert_aggregate_finished(&embedded_events, "step_completed");

    let facade_path = directory.path().join("facade.sqlite3");
    let facade_handler = RecordingAggregationHandler::majority();
    let (facade_instance, facade_history) =
        Box::pin(run_facade(&facade_path, &yaml, facade_handler.clone())).await;
    assert_eq!(facade_instance.current_state().as_str(), "DONE");
    let expected_output = aggregate_output_from_mcp(structured(&embedded_instance));
    assert_eq!(aggregate_output(&facade_instance), expected_output);
    assert_eq!(facade_history, semantic_history(&embedded_events));

    drop(embedded);
    let reopened = EmbeddedMade::open(&embedded_path).unwrap();
    let reopened_instance = reopened
        .instance(&CeremonyId::new(SESSION).unwrap())
        .await
        .unwrap();
    assert_eq!(reopened_instance.current_state().as_str(), "DONE");
    assert_eq!(aggregate_output(&reopened_instance), expected_output);
    assert_eq!(
        reopened
            .audit_records(&CeremonyId::new(SESSION).unwrap())
            .await
            .unwrap()
            .last()
            .unwrap()
            .event()
            .unwrap()
            .event_type()
            .as_str(),
        "ceremony_completed"
    );

    (
        vec![remote_handler, embedded_handler, facade_handler],
        expected_output,
    )
}

#[tokio::test]
async fn vote_has_exact_remote_embedded_and_facade_execution_parity() {
    let (handlers, output) = Box::pin(execute_parity("vote")).await;
    assert_eq!(output, json!({"choice": "ship"}));
    for handler in handlers {
        assert_vote_handler_calls(&handler);
    }
}

#[tokio::test]
async fn synthesis_has_exact_remote_embedded_and_facade_execution_parity() {
    let (handlers, output) = Box::pin(execute_parity("synthesize")).await;
    assert_eq!(
        output,
        json!({
            "inputs": ["alpha", "beta", "gamma"],
            "summary": "synthesized",
            "winner_content": "synthesized"
        })
    );
    for handler in handlers {
        assert_synthesis_handler_calls(&handler);
    }
}

#[tokio::test]
async fn invalid_vote_reopens_as_failed_instead_of_stranded_in_progress() {
    let directory = scratch();
    let path = directory.path().join("invalid.sqlite3");
    let handler = RecordingAggregationHandler::split();
    let engine = durable_engine(&path, handler.clone());
    let definition = CeremonyDefinitionYaml::parse_str(&ceremony("vote")).unwrap();

    let error = engine
        .run(RunCeremonyInput::new(
            CeremonyId::new(SESSION).unwrap(),
            definition,
            CeremonyContext::empty(),
            LeaseOwnerId::new("aggregation-host").unwrap(),
            DurationMs::from_millis(60_000),
            "aggregation-test",
            AuditActorKind::Service,
        ))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("did not complete"));
    assert_vote_handler_calls(&handler);
    drop(engine);

    let reopened = EmbeddedMade::open(&path).unwrap();
    let id = CeremonyId::new(SESSION).unwrap();
    let instance = reopened.instance(&id).await.unwrap();
    let aggregate = instance
        .step_record(&StepId::new("aggregate").unwrap())
        .unwrap();
    assert_eq!(aggregate.status(), StepStatus::Failed);
    assert!(!aggregate.has_live_lease_at(PARITY_INSTANT));
    let records = reopened.audit_records(&id).await.unwrap();
    let aggregate_events = records
        .iter()
        .filter_map(|record| record.event())
        .filter(|event| match event {
            CeremonyEvent::StepStarted(started) => started.step_id.as_str() == "aggregate",
            CeremonyEvent::StepFailed(failed) => failed.step_id.as_str() == "aggregate",
            _ => false,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        aggregate_events.as_slice(),
        [CeremonyEvent::StepStarted(_), CeremonyEvent::StepFailed(_)]
    ));
}
