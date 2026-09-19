#![cfg(feature = "embedded")]

//! The durable embedded backend, exercised the way a client meets it: two
//! processes, one state file, and the question of what the second one can
//! still see.

use std::io::Write;
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
use made_mcp::{MadeMcpServer, EMBEDDED_STORE_PATH_ENV, EVENT_SINK_PATH_ENV, MCP_BACKEND_ENV};
use serde_json::{json, Value};

const CEREMONY_YAML: &str = r#"
version: "1.0"
name: "restart_survival"
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
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
      - request_intervention
      - respond_to_intervention
"#;
const AUTH_POLICY_ID: &str = "embedded-test";
const AUTH_TRUSTED_HOST_ID: &str = "embedded-test-host";
const SEARCH_STORE_ID: &str = "embedded-stdio-test-store";
const SEARCH_CURSOR_KEY: &str = "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5";

#[test]
fn an_event_sink_recovers_pending_records_before_reading_stdio() {
    let state = tempfile::tempdir().unwrap();
    let store = state.path().join("ceremonies.sqlite3");
    let sink = state.path().join("ceremony-events.jsonl");

    let first = run_made_mcp_process(
        &store,
        &[tool_call(
            1,
            "made_start_ceremony",
            &json!({
                "ceremony_id": "pending-publication",
                "definition_yaml": CEREMONY_YAML,
                "actor_id": "restart-smoke",
                "actor_kind": "service"
            }),
        )],
    );
    assert_ne!(first[0]["result"]["isError"], Value::Bool(true));
    assert!(
        !sink.exists(),
        "the first process had no transport and cannot have acknowledged delivery"
    );

    let recovered = run_made_mcp_process_with_event_sink(&store, &sink, &[]);
    assert!(
        recovered.is_empty(),
        "the recovery process received no calls"
    );
    let lines = std::fs::read_to_string(&sink).unwrap();
    let records = lines.lines().collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        2,
        "startup must drain the pending event and append its registry snapshot"
    );
    let record: Value = serde_json::from_str(records[0]).unwrap();
    assert_eq!(record["global_position"], 1);
    assert_eq!(record["record"]["ceremony_id"], "pending-publication");
    assert_eq!(record["record"]["event_type"], "ceremony_instance_started");
    assert_eq!(record["schema_version"], 3);
    let metrics: Value = serde_json::from_str(records[1]).unwrap();
    assert_eq!(metrics["record_type"], "metrics_snapshot");
    assert!(metrics["registry_text"].is_string());
    assert!(metrics["registry"].is_array());

    // The cursor was acknowledged by recovery. Reopening again is a no-op,
    // rather than duplicate delivery of the same at-least-once attempt.
    run_made_mcp_process_with_event_sink(&store, &sink, &[]);
    assert_eq!(std::fs::read_to_string(&sink).unwrap().lines().count(), 2);
}

#[test]
fn a_decision_is_recalled_by_the_next_made_mcp_process() {
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("ceremonies.sqlite3");
    let scope = "team:stdio-restart";

    let first = run_made_mcp_process(
        &path,
        &[
            tool_call(
                1,
                "made_start_ceremony",
                &json!({
                    "ceremony_id": "memory-first",
                    "definition_yaml": CEREMONY_YAML,
                    "actor_id": "restart-smoke",
                    "actor_kind": "service",
                    "context": { "memory_scope": scope },
                }),
            ),
            tool_call(
                2,
                "made_request_ceremony_intervention",
                &json!({
                    "ceremony_id": "memory-first",
                    "intervention_id": "restart-decision",
                    "role_id": "FACILITATOR",
                    "role_kind": "human",
                    "kind": "opinion",
                    "message": "Which recovery do we rehearse?",
                }),
            ),
            tool_call(
                3,
                "made_respond_to_ceremony_intervention",
                &json!({
                    "ceremony_id": "memory-first",
                    "intervention_id": "restart-decision",
                    "role_id": "FACILITATOR",
                    "role_kind": "human",
                    "message": "Roll back rather than restart.",
                }),
            ),
        ],
    );
    assert_eq!(first.len(), 3, "the first process must answer every call");
    assert_ne!(
        first[2]["result"]["isError"],
        Value::Bool(true),
        "{:#}",
        first[2]
    );

    let second = run_made_mcp_process(
        &path,
        &[tool_call(
            1,
            "made_start_ceremony",
            &json!({
                "ceremony_id": "memory-second",
                "definition_yaml": CEREMONY_YAML,
                "actor_id": "restart-smoke",
                "actor_kind": "service",
                "context": { "memory_scope": scope },
            }),
        )],
    );
    let recollection = &second[0]["result"]["structuredContent"]["recollection"];
    assert_eq!(recollection["scope"], scope);
    assert!(
        recollection["entries"]
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| {
                entry["kind"] == "decision"
                    && entry["summary"] == "Roll back rather than restart."
                    && entry["from_ceremony_id"] == "memory-first"
            })),
        "the second process did not recall the first process's decision: {:#}",
        second[0]
    );
}

#[tokio::test]
async fn a_started_ceremony_is_read_back_by_the_next_process() {
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("ceremonies.sqlite3");

    let ceremony_id = {
        let first = MadeMcpServer::embedded_sqlite(&path).expect("the store must open");

        let published = send(
            &first,
            tool_call(
                1,
                "made_publish_ceremony_definition",
                &json!({ "definition_yaml": CEREMONY_YAML }),
            ),
        )
        .await;
        assert_ne!(
            published["result"]["isError"],
            Value::Bool(true),
            "{published:?}"
        );

        let started = send(
            &first,
            tool_call(
                2,
                "made_start_published_ceremony",
                &json!({
                    "ceremony": "restart_survival",
                    "version": "1.0",
                    "actor_id": "restart-smoke",
                    "actor_kind": "service"
                }),
            ),
        )
        .await;
        assert_ne!(
            started["result"]["isError"],
            Value::Bool(true),
            "{started:?}"
        );

        let instance = &started["result"]["structuredContent"];
        assert_eq!(instance["current_state"], "OPEN");
        instance["ceremony_id"].as_str().unwrap().to_owned()
    };

    // The first server is gone. Everything below is a cold read of the file.
    let second = MadeMcpServer::embedded_sqlite(&path).expect("the store must reopen");

    let recovered = send(
        &second,
        tool_call(
            1,
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": ceremony_id }),
        ),
    )
    .await;
    assert_ne!(
        recovered["result"]["isError"],
        Value::Bool(true),
        "{recovered:?}"
    );

    let instance = &recovered["result"]["structuredContent"];
    assert_eq!(instance["ceremony_id"], ceremony_id.as_str());
    assert_eq!(instance["definition_name"], "restart_survival");
    assert_eq!(instance["definition_version"], "1.0");
    assert_eq!(instance["current_state"], "OPEN");
    assert_eq!(instance["next_step_id"], "work");
    assert_eq!(instance["completed"], false);
    // The instance is still bound to the exact published definition it
    // started from, not merely to a name that happens to match.
    assert!(instance["bound_definition_digest"]
        .as_str()
        .is_some_and(|digest| !digest.is_empty()));

    let listed = send(
        &second,
        tool_call(2, "made_list_ceremony_instances", &json!({})),
    )
    .await;
    assert_eq!(listed["result"]["structuredContent"]["count"], 1);
}

#[tokio::test]
async fn a_separate_state_file_does_not_see_another_one_s_ceremonies() {
    let state = tempfile::tempdir().unwrap();

    let first = MadeMcpServer::embedded_sqlite(state.path().join("one.sqlite3")).unwrap();
    let published = send(
        &first,
        tool_call(
            1,
            "made_publish_ceremony_definition",
            &json!({ "definition_yaml": CEREMONY_YAML }),
        ),
    )
    .await;
    assert_ne!(
        published["result"]["isError"],
        Value::Bool(true),
        "{published:?}"
    );
    send(
        &first,
        tool_call(
            2,
            "made_start_published_ceremony",
            &json!({
                "ceremony": "restart_survival",
                "version": "1.0",
                "actor_id": "restart-smoke",
                "actor_kind": "service"
            }),
        ),
    )
    .await;

    let other = MadeMcpServer::embedded_sqlite(state.path().join("two.sqlite3")).unwrap();
    let listed = send(
        &other,
        tool_call(1, "made_list_ceremony_instances", &json!({})),
    )
    .await;
    assert_eq!(
        listed["result"]["structuredContent"]["count"], 0,
        "state must come from the file the caller named, not from the process"
    );
}

#[tokio::test]
async fn an_instance_that_cannot_rehydrate_is_reported_without_hiding_the_ones_that_can() {
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("ceremonies.sqlite3");

    {
        let first = MadeMcpServer::embedded_sqlite(&path).unwrap();
        // A one-shot run mounts its definition for this process only: the
        // instance is committed to the store, the definition is not.
        let ran = send(
            &first,
            tool_call(
                1,
                "made_run_ceremony",
                &json!({
                    "ceremony_id": "one-shot",
                    "definition_yaml": CEREMONY_YAML,
                    "actor_id": "restart-smoke",
                    "actor_kind": "service"
                }),
            ),
        )
        .await;
        assert_ne!(ran["result"]["isError"], Value::Bool(true), "{ran:?}");

        let published = send(
            &first,
            tool_call(
                2,
                "made_publish_ceremony_definition",
                &json!({ "definition_yaml": CEREMONY_YAML }),
            ),
        )
        .await;
        assert_ne!(
            published["result"]["isError"],
            Value::Bool(true),
            "{published:?}"
        );
        let started = send(
            &first,
            tool_call(
                3,
                "made_start_published_ceremony",
                &json!({
                    "ceremony": "restart_survival",
                    "version": "1.0",
                    "ceremony_id": "published-one",
                    "actor_id": "restart-smoke",
                    "actor_kind": "service"
                }),
            ),
        )
        .await;
        assert_ne!(
            started["result"]["isError"],
            Value::Bool(true),
            "{started:?}"
        );
    }

    let second = MadeMcpServer::embedded_sqlite(&path).unwrap();
    let listed = send(
        &second,
        tool_call(1, "made_list_ceremony_instances", &json!({})),
    )
    .await;
    assert_ne!(listed["result"]["isError"], Value::Bool(true), "{listed:?}");

    let instances = listed["result"]["structuredContent"]["instances"]
        .as_array()
        .unwrap();
    let orphan = instances
        .iter()
        .find(|entry| entry["ceremony_id"] == "one-shot")
        .expect("the one-shot instance is still stored");
    assert_eq!(orphan["rehydratable"], false);
    assert!(orphan["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("definition")));

    let recovered = instances
        .iter()
        .find(|entry| entry["ceremony_id"] == "published-one")
        .expect("the published instance rehydrates");
    assert_eq!(recovered["current_state"], "OPEN");
    assert_eq!(recovered["definition_name"], "restart_survival");

    // Asking for the orphan by name still fails: the listing degrades, the
    // direct read does not pretend.
    let direct = send(
        &second,
        tool_call(
            2,
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": "one-shot" }),
        ),
    )
    .await;
    assert_eq!(direct["result"]["isError"], Value::Bool(true), "{direct:?}");
}

#[tokio::test]
async fn opening_the_store_over_a_directory_fails_instead_of_degrading_to_memory() {
    let state = tempfile::tempdir().unwrap();
    let Err(error) = MadeMcpServer::embedded_sqlite(state.path()) else {
        panic!("a directory is not a ceremony store");
    };
    assert!(
        error.contains("embedded SQLite ceremony store"),
        "the failure must name what did not open: {error}"
    );
}

#[test]
fn the_embedded_backend_selected_by_env_requires_a_state_file() {
    // Exercise environment selection in child processes. Changing this
    // process's policy environment races with in-process SQLite fixtures.
    let refused = Command::new(env!("CARGO_BIN_EXE_made-mcp"))
        .env(MCP_BACKEND_ENV, "embedded")
        .env_remove(EMBEDDED_STORE_PATH_ENV)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "embedded must demand a state file"
    );
    let message = String::from_utf8(refused.stderr).unwrap();
    assert!(message.contains(EMBEDDED_STORE_PATH_ENV), "{message}");
    let state = tempfile::tempdir().unwrap();
    let responses = run_made_mcp_process(
        &state.path().join("ceremonies.sqlite3"),
        &[json!({"jsonrpc":"2.0", "id":1, "method":"initialize"})],
    );
    assert_eq!(responses[0]["result"]["metadata"]["backend"], "embedded");
}

async fn send(server: &MadeMcpServer, request: Value) -> Value {
    let response = server
        .handle_json_line(&request.to_string())
        .await
        .expect("request must produce a response");
    serde_json::from_str(&response).unwrap()
}

fn tool_call(id: u64, tool: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    })
}

fn run_made_mcp_process(path: &std::path::Path, requests: &[Value]) -> Vec<Value> {
    run_made_mcp_process_with_optional_event_sink(path, None, requests)
}

fn run_made_mcp_process_with_event_sink(
    path: &std::path::Path,
    sink: &std::path::Path,
    requests: &[Value],
) -> Vec<Value> {
    run_made_mcp_process_with_optional_event_sink(path, Some(sink), requests)
}

fn run_made_mcp_process_with_optional_event_sink(
    path: &std::path::Path,
    sink: Option<&std::path::Path>,
    requests: &[Value],
) -> Vec<Value> {
    bootstrap_authorization(path);
    let mut command = Command::new(env!("CARGO_BIN_EXE_made-mcp"));
    command
        .env(MCP_BACKEND_ENV, "embedded")
        .env(EMBEDDED_STORE_PATH_ENV, path)
        .env("MADE_AUTH_POLICY_ID", AUTH_POLICY_ID)
        .env("MADE_AUTH_TRUSTED_HOST_ID", AUTH_TRUSTED_HOST_ID)
        .env("MADE_CEREMONY_STORE_ID", SEARCH_STORE_ID)
        .env("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", SEARCH_CURSOR_KEY)
        .env_remove(EVENT_SINK_PATH_ENV);
    if let Some(sink) = sink {
        command.env(EVENT_SINK_PATH_ENV, sink);
    }
    let mut child = command
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

fn bootstrap_authorization(path: &std::path::Path) {
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
                AuthorizationGrantId::new("embedded-test-all").unwrap(),
                owner.id().clone(),
                [
                    AuthorizationAction::StartCeremony,
                    AuthorizationAction::StartPublishedCeremony,
                    AuthorizationAction::GetCeremonyInstance,
                    AuthorizationAction::ListCeremonyInstances,
                    AuthorizationAction::PublishCeremonyDefinition,
                    AuthorizationAction::ReadCeremonyEvents,
                    AuthorizationAction::RunCeremony,
                    AuthorizationAction::RequestCeremonyIntervention,
                    AuthorizationAction::RespondToCeremonyIntervention,
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
