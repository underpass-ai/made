#![cfg(feature = "embedded")]

//! An intervention crossing from a supervisor to a working agent, with
//! two real processes and one store between them.
//!
//! Not a fixture and not one process pretending to be two. The
//! supervisor and the worker are separate `made-mcp` executables
//! speaking JSON-RPC over their own stdin and stdout, sharing only the
//! SQLite file underneath, because the thing under test is whether a
//! question put down by one process is found by another — and an
//! in-process test cannot fail that way.
//!
//! What the ledger is for shows up in the negative cases: a host that
//! refuses, one whose incarnation was replaced, one that acknowledges
//! twice, and a ceremony that ends with a question still in flight.
//! Each of those is a state an operator has to be able to read
//! afterwards, and none of them is the same as "delivered".

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::AuthorizationPolicyAdministrationService;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationGrant,
    AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope,
    DelegationDepth, PrincipalId, PrincipalKind,
};
use made_mcp::{EMBEDDED_STORE_PATH_ENV, EVENT_SINK_PATH_ENV, MCP_BACKEND_ENV};
use serde_json::{json, Value};

const CEREMONY_YAML: &str = r#"
version: "1.0"
name: "intervention_bridge"
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
    handler: embedded_noop
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

const AUTH_POLICY_ID: &str = "bridge-test";
const AUTH_TRUSTED_HOST_ID: &str = "bridge-test-host";
const SEARCH_STORE_ID: &str = "bridge-stdio-test-store";
const SEARCH_CURSOR_KEY: &str =
    "b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5";

const CEREMONY: &str = "bridge-1";
const EXECUTION: &str = "exec-1";
const INCARNATION: &str = "inc-1";
const REPLACEMENT: &str = "inc-2";
const ITEM: &str = "item-1";

#[test]
fn a_supervisor_and_a_worker_in_separate_processes_carry_one_question_to_an_answer() {
    let state = tempfile::tempdir().unwrap();
    let store = state.path().join("ceremonies.sqlite3");
    bootstrap_authorization(&store);

    // The worker process claims the step and reports itself live, which
    // is what makes it a destination anything can be addressed to.
    let claim = worker_process(&store, &[start(1), claim_step(2), report_status(3, INCARNATION)]);
    assert_ok(&claim);
    let claim_fence = structured(&claim[1])["claim_fence"]
        .as_str()
        .expect("the claim answers with its fence")
        .to_owned();

    // The supervisor process, which has never met the worker, puts a
    // question to that exact execution.
    let asked = supervisor_process(&store, &[request_exact(4, ITEM, INCARNATION)]);
    assert_ok(&asked);

    // Before anything is offered, the question is recorded and no more.
    let recorded = supervisor_process(&store, &[get_intervention(5, ITEM)]);
    assert_ok(&recorded);
    let status = structured(&recorded[0])["status_delivery"]
        .as_str()
        .expect("the projection names a status");
    assert!(
        matches!(status, "recorded" | "queued"),
        "an item nobody has been handed said {status}"
    );
    assert!(
        structured(&recorded[0])["deliveries"]
            .as_array()
            .expect("an acknowledgement list")
            .is_empty(),
        "nobody had acknowledged anything yet"
    );

    // The worker pulls, which is a lease and not a receipt.
    let pulled = worker_process(&store, &[report_status(6, INCARNATION), pull(7, INCARNATION)]);
    assert_ok(&pulled);
    let items = structured(&pulled[1])["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "the worker was handed exactly its question");
    let delivery_id = items[0]["delivery_id"].as_str().unwrap().to_owned();
    let lease_id = items[0]["lease_id"].as_str().unwrap().to_owned();
    assert_eq!(
        items[0]["intervention"]["intervention_id"], ITEM,
        "the lease carries the item itself, not only its id"
    );

    // The supervisor reads `delivered`, and the acknowledgement list is
    // still empty: a lease says the engine handed it over, and nothing
    // more than that.
    let delivered = supervisor_process(&store, &[get_intervention(8, ITEM)]);
    assert_eq!(structured(&delivered[0])["status_delivery"], "delivered");
    assert!(structured(&delivered[0])["deliveries"]
        .as_array()
        .unwrap()
        .is_empty());

    // The worker says what it saw. This is the first evidence anybody
    // has that the question arrived.
    let acknowledged = worker_process(
        &store,
        &[acknowledge(9, &delivery_id, &lease_id, INCARNATION, "received")],
    );
    assert_ok(&acknowledged);

    let seen = supervisor_process(&store, &[get_intervention(10, ITEM)]);
    assert_eq!(structured(&seen[0])["status_delivery"], "acknowledged");
    let acks = structured(&seen[0])["deliveries"].as_array().unwrap();
    assert_eq!(acks.len(), 1, "one acknowledgement, sealed in the stream");
    assert_eq!(acks[0]["incarnation"], INCARNATION);
    assert_eq!(acks[0]["observation_kind"], "received");

    // An acknowledgement repeated, because the host lost its answer, is
    // the same fact and not a second one.
    let repeated = worker_process(
        &store,
        &[acknowledge(11, &delivery_id, &lease_id, INCARNATION, "received")],
    );
    assert_ok(&repeated);
    let after_repeat = supervisor_process(&store, &[get_intervention(12, ITEM)]);
    assert_eq!(
        structured(&after_repeat[0])["deliveries"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "repeating an acknowledgement appended a second one"
    );

    // And the answer, attributed to the agent that was handed it.
    let answered = worker_process(
        &store,
        &[respond(13, &delivery_id, INCARNATION, &claim_fence)],
    );
    assert_ok(&answered);

    let resolved = supervisor_process(&store, &[get_intervention(14, ITEM), list(15)]);
    assert_eq!(structured(&resolved[0])["status_delivery"], "responded");
    assert_eq!(structured(&resolved[0])["unresolved"], false);
    let response = &structured(&resolved[0])["responses"][0];
    assert_eq!(response["executor_incarnation"], INCARNATION);
    assert_eq!(response["delivery_id"], delivery_id.as_str());
    let listed = structured(&resolved[1])["interventions"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "the ceremony holds the one question asked");
}

#[test]
fn a_replaced_agent_is_refused_and_a_refusal_is_visible() {
    let state = tempfile::tempdir().unwrap();
    let store = state.path().join("ceremonies.sqlite3");
    bootstrap_authorization(&store);

    let claim = worker_process(&store, &[start(1), claim_step(2), report_status(3, INCARNATION)]);
    assert_ok(&claim);
    let asked = supervisor_process(&store, &[request_exact(4, ITEM, INCARNATION)]);
    assert_ok(&asked);

    // A different generation of the same execution asks for the
    // questions its predecessor was asked. The roster says the live
    // incarnation is `inc-1`, so this is refused before anything is
    // handed over.
    let foreign = worker_process(&store, &[pull(5, REPLACEMENT)]);
    assert_eq!(
        foreign[0]["result"]["isError"],
        Value::Bool(true),
        "a replacement process was handed its predecessor's question: {:?}",
        foreign[0]
    );

    let untouched = supervisor_process(&store, &[get_intervention(6, ITEM)]);
    let status = structured(&untouched[0])["status_delivery"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        matches!(status.as_str(), "recorded" | "queued"),
        "a refused pull moved the item to {status}"
    );

    // The right agent takes it and declines it. A refusal is an
    // observation about the item, so it is sealed — and it is not a
    // delivery anybody is still waiting on.
    let pulled = worker_process(&store, &[report_status(7, INCARNATION), pull(8, INCARNATION)]);
    assert_ok(&pulled);
    let items = structured(&pulled[1])["items"].as_array().unwrap();
    let delivery_id = items[0]["delivery_id"].as_str().unwrap().to_owned();
    let lease_id = items[0]["lease_id"].as_str().unwrap().to_owned();

    let refused = worker_process(
        &store,
        &[acknowledge(9, &delivery_id, &lease_id, INCARNATION, "refused")],
    );
    assert_ok(&refused);

    let read = supervisor_process(&store, &[get_intervention(10, ITEM)]);
    let view = structured(&read[0]);
    assert_eq!(view["status_delivery"], "failed");
    assert_eq!(view["unresolved"], false);
    assert_eq!(view["deliveries"][0]["observation_kind"], "refused");
    assert!(
        view["status_reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("refused")),
        "a refusal left no reason behind: {view}"
    );
}

#[test]
fn a_conflicting_second_answer_about_one_offer_is_refused() {
    let state = tempfile::tempdir().unwrap();
    let store = state.path().join("ceremonies.sqlite3");
    bootstrap_authorization(&store);

    let claim = worker_process(&store, &[start(1), claim_step(2), report_status(3, INCARNATION)]);
    assert_ok(&claim);
    assert_ok(&supervisor_process(
        &store,
        &[request_exact(4, ITEM, INCARNATION)],
    ));

    let pulled = worker_process(&store, &[pull(5, INCARNATION)]);
    assert_ok(&pulled);
    let items = structured(&pulled[0])["items"].as_array().unwrap();
    let delivery_id = items[0]["delivery_id"].as_str().unwrap().to_owned();
    let lease_id = items[0]["lease_id"].as_str().unwrap().to_owned();

    assert_ok(&worker_process(
        &store,
        &[acknowledge(6, &delivery_id, &lease_id, INCARNATION, "received")],
    ));
    // Changing its mind about what it saw. The first statement is
    // already sealed, and the stream does not overwrite.
    let conflicting = worker_process(
        &store,
        &[acknowledge(7, &delivery_id, &lease_id, INCARNATION, "incapable")],
    );
    assert_eq!(
        conflicting[0]["result"]["isError"],
        Value::Bool(true),
        "a host rewrote what it had already said: {:?}",
        conflicting[0]
    );

    let read = supervisor_process(&store, &[get_intervention(8, ITEM)]);
    let acks = structured(&read[0])["deliveries"].as_array().unwrap();
    assert_eq!(acks.len(), 1);
    assert_eq!(acks[0]["observation_kind"], "received");
}

#[test]
fn a_ceremony_that_ends_leaves_no_question_waiting() {
    let state = tempfile::tempdir().unwrap();
    let store = state.path().join("ceremonies.sqlite3");
    bootstrap_authorization(&store);

    let claim = worker_process(&store, &[start(1), claim_step(2), report_status(3, INCARNATION)]);
    assert_ok(&claim);
    assert_ok(&supervisor_process(
        &store,
        &[request_exact(4, ITEM, INCARNATION)],
    ));

    // The supervisor cancels with the question still in flight.
    let ended = supervisor_process(&store, &[cancel(5)]);
    assert_ok(&ended);

    let read = supervisor_process(&store, &[get_intervention(6, ITEM)]);
    let view = structured(&read[0]);
    assert_eq!(
        view["status_delivery"], "expired",
        "a question nobody was ever handed survived its ceremony: {view}"
    );
    assert_eq!(view["status_reason"], "ceremony_ended");
    assert_eq!(view["unresolved"], false);
}

fn start(id: u64) -> Value {
    tool_call(
        id,
        "made_start_ceremony",
        &json!({"ceremony_id": CEREMONY, "definition_yaml": CEREMONY_YAML}),
    )
}

fn claim_step(id: u64) -> Value {
    tool_call(
        id,
        "made_claim_ceremony_step",
        &json!({
            "ceremony_id": CEREMONY,
            "step_id": "work",
            "role_id": "ENGINEER",
            "lease_owner_id": "bridge-worker",
            "idempotency_key": "bridge-claim-1"
        }),
    )
}

fn report_status(id: u64, incarnation: &str) -> Value {
    tool_call(
        id,
        "made_report_ceremony_agent_status",
        &json!({"status": {
            "ceremony_id": CEREMONY,
            "agent_execution_id": EXECUTION,
            "operation_id": operation_id(),
            "claim_owner_id": "bridge-worker",
            "logical_worker_id": "bridge-worker",
            "host_agent_id": "bridge-worker",
            "host_agent_incarnation": incarnation,
            "role_id": "ENGINEER",
            "step_id": "work",
            "attempt": 1,
            "execution_status": "running",
            "liveness": "fresh",
            "source": "host_report",
            "activity": "working on the step",
            "task_summary": "the bridge fixture's only step",
            "evidence_references": [],
            "observed_at": "2026-09-20T12:00:00Z",
            "report_sequence": 1,
            "idempotency_key": format!("bridge-status-{incarnation}"),
            "claim_fence": claim_fence_placeholder()
        }}),
    )
}

fn request_exact(id: u64, item: &str, incarnation: &str) -> Value {
    tool_call(
        id,
        "made_request_ceremony_intervention",
        &json!({
            "ceremony_id": CEREMONY,
            "intervention_id": item,
            "role_id": "LEAD",
            "role_kind": "human",
            "kind": "opinion",
            "intent": "question",
            "target_agent_execution_id": EXECUTION,
            "target_incarnation": incarnation,
            "message": "Is the migration still reversible?"
        }),
    )
}

fn pull(id: u64, incarnation: &str) -> Value {
    tool_call(
        id,
        "made_pull_ceremony_agent_interventions",
        &json!({
            "ceremony_id": CEREMONY,
            "agent_execution_id": EXECUTION,
            "incarnation": incarnation,
            "role_id": "ENGINEER"
        }),
    )
}

fn acknowledge(
    id: u64,
    delivery_id: &str,
    lease_id: &str,
    incarnation: &str,
    observation: &str,
) -> Value {
    tool_call(
        id,
        "made_acknowledge_ceremony_agent_intervention",
        &json!({
            "ceremony_id": CEREMONY,
            "intervention_id": ITEM,
            "delivery_id": delivery_id,
            "lease_id": lease_id,
            "agent_execution_id": EXECUTION,
            "incarnation": incarnation,
            "role_id": "ENGINEER",
            "observation_kind": observation,
            "note": "the bridge fixture's worker"
        }),
    )
}

fn respond(id: u64, delivery_id: &str, incarnation: &str, _claim_fence: &str) -> Value {
    tool_call(
        id,
        "made_respond_to_ceremony_intervention",
        &json!({
            "ceremony_id": CEREMONY,
            "intervention_id": ITEM,
            "role_id": "ENGINEER",
            "role_kind": "agent",
            "message": "Yes, the migration is still reversible.",
            "delivery_id": delivery_id,
            "agent_execution_id": EXECUTION,
            "incarnation": incarnation
        }),
    )
}

fn get_intervention(id: u64, item: &str) -> Value {
    tool_call(
        id,
        "made_get_ceremony_intervention",
        &json!({"ceremony_id": CEREMONY, "intervention_id": item}),
    )
}

fn list(id: u64) -> Value {
    tool_call(
        id,
        "made_list_ceremony_interventions",
        &json!({"ceremony_id": CEREMONY}),
    )
}

fn cancel(id: u64) -> Value {
    tool_call(
        id,
        "made_cancel_ceremony",
        &json!({
            "ceremony_id": CEREMONY,
            "reason": "the bridge fixture ends the ceremony with a question in flight"
        }),
    )
}

fn operation_id() -> String {
    format!("{CEREMONY}:work:1:1:1")
}

fn claim_fence_placeholder() -> String {
    format!("{CEREMONY}:work:1:1:1")
}

fn tool_call(id: u64, tool: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    })
}

fn structured(response: &Value) -> &Value {
    &response["result"]["structuredContent"]
}

fn assert_ok(responses: &[Value]) {
    for response in responses {
        assert_ne!(
            response["result"]["isError"],
            Value::Bool(true),
            "a request the bridge depends on failed: {response}"
        );
    }
}

/// The agent's own process, which never speaks to the supervisor's.
fn worker_process(store: &Path, requests: &[Value]) -> Vec<Value> {
    run_made_mcp_process(store, requests)
}

/// The supervisor's process, which never speaks to the worker's.
fn supervisor_process(store: &Path, requests: &[Value]) -> Vec<Value> {
    run_made_mcp_process(store, requests)
}

fn run_made_mcp_process(path: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_made-mcp"))
        .env(MCP_BACKEND_ENV, "embedded")
        .env(EMBEDDED_STORE_PATH_ENV, path)
        .env("MADE_AUTH_POLICY_ID", AUTH_POLICY_ID)
        .env("MADE_AUTH_TRUSTED_HOST_ID", AUTH_TRUSTED_HOST_ID)
        .env("MADE_CEREMONY_STORE_ID", SEARCH_STORE_ID)
        .env("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", SEARCH_CURSOR_KEY)
        .env_remove(EVENT_SINK_PATH_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("made-mcp process starts");
    {
        let input = child.stdin.as_mut().expect("stdin is piped");
        for request in requests {
            writeln!(input, "{request}").expect("request writes");
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("made-mcp process exits");
    assert!(
        output.status.success(),
        "made-mcp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn bootstrap_authorization(path: &Path) {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let store = Arc::new(SqliteAuthorizationPolicyStore::open(path).unwrap());
            let service = AuthorizationPolicyAdministrationService::new(
                AuthorizationPolicyId::new(AUTH_POLICY_ID).unwrap(),
                store,
                Arc::new(SystemClock::new()),
            );
            let owner = AuthenticatedPrincipal::new(
                PrincipalId::new(AUTH_TRUSTED_HOST_ID).unwrap(),
                PrincipalKind::TrustedHost,
                AuthenticationMethod::LocalHostPolicy,
            )
            .unwrap();
            service.open(owner.clone(), Vec::new()).await.unwrap();
            let grant = AuthorizationGrant::new(
                AuthorizationGrantId::new("bridge-test-all").unwrap(),
                owner.id().clone(),
                [
                    AuthorizationAction::StartCeremony,
                    AuthorizationAction::GetCeremonyInstance,
                    AuthorizationAction::ClaimCeremonyStep,
                    AuthorizationAction::ReportCeremonyAgentStatus,
                    AuthorizationAction::CancelCeremony,
                    AuthorizationAction::RequestCeremonyIntervention,
                    AuthorizationAction::RespondToCeremonyIntervention,
                    AuthorizationAction::PullCeremonyAgentInterventions,
                    AuthorizationAction::AcknowledgeCeremonyAgentIntervention,
                    AuthorizationAction::GetCeremonyIntervention,
                    AuthorizationAction::ListCeremonyInterventions,
                ],
                AuthorizationScope::Global,
                (time::OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap();
            service.issue(&owner, grant).await.unwrap();
        });
    })
    .join()
    .unwrap();
}
