#![cfg(feature = "embedded")]

//! The durable embedded backend, exercised the way a client meets it: two
//! processes, one state file, and the question of what the second one can
//! still see.

use made_mcp::{MadeMcpServer, EMBEDDED_REDB_PATH_ENV, LEGACY_REDB_PATH_ENV, MCP_BACKEND_ENV};
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
"#;

#[tokio::test]
async fn a_started_ceremony_is_read_back_by_the_next_process() {
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("ceremonies.redb");

    let ceremony_id = {
        let first = MadeMcpServer::embedded_redb(&path).expect("the store must open");

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
    let second = MadeMcpServer::embedded_redb(&path).expect("the store must reopen");

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

    let first = MadeMcpServer::embedded_redb(state.path().join("one.redb")).unwrap();
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

    let other = MadeMcpServer::embedded_redb(state.path().join("two.redb")).unwrap();
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
    let path = state.path().join("ceremonies.redb");

    {
        let first = MadeMcpServer::embedded_redb(&path).unwrap();
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

    let second = MadeMcpServer::embedded_redb(&path).unwrap();
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
    let Err(error) = MadeMcpServer::embedded_redb(state.path()) else {
        panic!("a directory is not a ceremony store");
    };
    assert!(
        error.contains("embedded redb ceremony store"),
        "the failure must name what did not open: {error}"
    );
}

#[test]
fn the_embedded_backend_selected_by_env_requires_a_state_file() {
    // One test owns the process environment for both directions: the
    // variables are global, so splitting this would race with itself.
    std::env::set_var(MCP_BACKEND_ENV, "embedded");
    std::env::remove_var(EMBEDDED_REDB_PATH_ENV);
    std::env::remove_var(LEGACY_REDB_PATH_ENV);

    let Err(refused) = MadeMcpServer::try_from_env() else {
        panic!("embedded must demand a state file");
    };
    assert!(refused.contains(EMBEDDED_REDB_PATH_ENV), "{refused}");

    let state = tempfile::tempdir().unwrap();
    std::env::set_var(EMBEDDED_REDB_PATH_ENV, state.path().join("ceremonies.redb"));
    let Ok(server) = MadeMcpServer::try_from_env() else {
        panic!("a named state file must be accepted");
    };
    assert_eq!(server.backend_name(), "embedded");

    std::env::remove_var(MCP_BACKEND_ENV);
    std::env::remove_var(EMBEDDED_REDB_PATH_ENV);
    std::env::remove_var(LEGACY_REDB_PATH_ENV);
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
