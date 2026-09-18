//! Host-owned fan-out through the embedded protocol and across SQLite processes.

use std::path::{Path, PathBuf};
use std::process::Command;

use made_embedded::EmbeddedMade;
use made_mcp::{EmbeddedMadeMcpBackend, MadeMcpServer};
use serde_json::{json, Value};

const CEREMONY: &str = r#"
version: "1.0"
name: host_fanout
states:
  - id: REVIEW
    initial: true
    execution: concurrent
  - id: DONE
    terminal: true
transitions:
  - from: REVIEW
    to: DONE
    trigger: finish
    guards: [both_done]
steps:
  - {id: review_api, state: REVIEW, handler: host_callback}
  - {id: review_data, state: REVIEW, handler: host_callback}
guards:
  both_done: {type: automated, check: "steps_completed:2"}
roles:
  - {id: API_REVIEWER, allowed_actions: [review_api, finish]}
  - {id: DATA_REVIEWER, allowed_actions: [review_data, finish]}
max_parallel: 2
"#;

fn scratch() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn server(path: &Path) -> MadeMcpServer {
    MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(
        EmbeddedMade::open(path).expect("SQLite embedded engine opens"),
    ))
}

async fn call(server: &MadeMcpServer, id: u64, tool: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    });
    let line = server.handle_json_line(&request.to_string()).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

fn structured(answer: &Value) -> &Value {
    &answer["result"]["structuredContent"]
}

fn assert_ok(answer: &Value) {
    assert!(answer.get("error").is_none(), "{answer:#}");
    assert_ne!(answer["result"]["isError"], true, "{answer:#}");
}

async fn open_session(path: &Path, id: &str) {
    let backend = server(path);
    let published = call(
        &backend,
        1,
        "made_publish_ceremony_definition",
        json!({"definition_yaml": CEREMONY}),
    )
    .await;
    assert_ok(&published);
    let started = call(
        &backend,
        2,
        "made_start_published_ceremony",
        json!({
            "ceremony_id": id, "ceremony": "host_fanout", "version": "1.0",
            "actor_id": "host", "actor_kind": "service"
        }),
    )
    .await;
    assert_ok(&started);
    assert_eq!(
        structured(&started)["claimable_step_ids"],
        json!(["review_api", "review_data"])
    );
}

async fn claim(server: &MadeMcpServer, id: u64, ceremony: &str, step: &str) -> Value {
    call(
        server,
        id,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": ceremony, "step_id": step, "actor_kind": "agent",
            "lease_owner_id": format!("host-{step}"),
            "idempotency_key": format!("claim-{step}"), "lease_ttl_ms": 60_000
        }),
    )
    .await
}

async fn complete(
    server: &MadeMcpServer,
    id: u64,
    ceremony: &str,
    step: &str,
    fence: &Value,
) -> Value {
    call(
        server,
        id,
        "made_complete_ceremony_step",
        json!({
            "ceremony_id": ceremony, "step_id": step, "actor_kind": "agent",
            "status": "completed", "output": {"worker": step}, "claim_fence": fence
        }),
    )
    .await
}

#[tokio::test]
async fn embedded_protocol_fans_out_two_claims_and_accepts_reverse_completion() {
    let directory = scratch();
    let path = directory.path().join("embedded.sqlite3");
    open_session(&path, "embedded-fanout").await;
    let backend = server(&path);

    let (api, data) = tokio::join!(
        claim(&backend, 10, "embedded-fanout", "review_api"),
        claim(&backend, 11, "embedded-fanout", "review_data")
    );
    assert_ok(&api);
    assert_ok(&data);
    assert_ne!(
        structured(&api)["claim_fence"],
        structured(&data)["claim_fence"]
    );

    let data_done = complete(
        &backend,
        12,
        "embedded-fanout",
        "review_data",
        &structured(&data)["claim_fence"],
    )
    .await;
    assert_ok(&data_done);
    let api_done = complete(
        &backend,
        13,
        "embedded-fanout",
        "review_api",
        &structured(&api)["claim_fence"],
    )
    .await;
    assert_ok(&api_done);
    assert_eq!(structured(&api_done)["transitions"][0]["enabled"], true);
}

#[tokio::test]
#[ignore = "spawned only by two_processes_claim_distinct_sqlite_steps"]
async fn sqlite_claim_worker() {
    let Ok(store) = std::env::var("MADE_FANOUT_STORE") else {
        return;
    };
    let ceremony = std::env::var("MADE_FANOUT_CEREMONY").unwrap();
    let step = std::env::var("MADE_FANOUT_STEP").unwrap();
    let ready = PathBuf::from(std::env::var("MADE_FANOUT_READY").unwrap());
    let go = PathBuf::from(std::env::var("MADE_FANOUT_GO").unwrap());
    let output = PathBuf::from(std::env::var("MADE_FANOUT_OUTPUT").unwrap());
    std::fs::write(&ready, b"ready").unwrap();
    while !go.exists() {
        tokio::task::yield_now().await;
    }
    let answer = claim(&server(Path::new(&store)), 1, &ceremony, &step).await;
    assert_ok(&answer);
    std::fs::write(output, serde_json::to_vec(structured(&answer)).unwrap()).unwrap();
}

#[tokio::test]
async fn two_processes_claim_distinct_sqlite_steps_and_complete_out_of_order() {
    let directory = scratch();
    let path = directory.path().join("processes.sqlite3");
    let ceremony = "process-fanout";
    open_session(&path, ceremony).await;

    let go = directory.path().join("go");
    let mut children = Vec::new();
    for step in ["review_api", "review_data"] {
        let ready = directory.path().join(format!("{step}.ready"));
        let output = directory.path().join(format!("{step}.json"));
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "sqlite_claim_worker", "--ignored", "--nocapture"])
            .env("MADE_FANOUT_STORE", &path)
            .env("MADE_FANOUT_CEREMONY", ceremony)
            .env("MADE_FANOUT_STEP", step)
            .env("MADE_FANOUT_READY", &ready)
            .env("MADE_FANOUT_GO", &go)
            .env("MADE_FANOUT_OUTPUT", &output)
            .spawn()
            .unwrap();
        children.push((step, ready, output, child));
    }
    while children.iter().any(|(_, ready, _, _)| !ready.exists()) {
        tokio::task::yield_now().await;
    }
    std::fs::write(&go, b"go").unwrap();
    for (_, _, _, child) in &mut children {
        assert!(child.wait().unwrap().success());
    }

    let claims = children
        .iter()
        .map(|(step, _, output, _)| {
            let value: Value = serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
            (*step, value)
        })
        .collect::<Vec<_>>();
    assert_ne!(claims[0].1["claim_fence"], claims[1].1["claim_fence"]);

    let backend = server(&path);
    for (id, (step, claim)) in claims.iter().rev().enumerate() {
        let answer = complete(
            &backend,
            20 + id as u64,
            ceremony,
            step,
            &claim["claim_fence"],
        )
        .await;
        assert_ok(&answer);
    }
    let instance = call(
        &backend,
        30,
        "made_get_ceremony_instance",
        json!({"ceremony_id": ceremony}),
    )
    .await;
    assert_ok(&instance);
    assert_eq!(structured(&instance)["transitions"][0]["enabled"], true);
    let history = call(
        &backend,
        30,
        "made_read_ceremony_events",
        json!({"ceremony_id": ceremony}),
    )
    .await;
    let completion_order = structured(&history)["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["event_type"] == "step_completed")
        .map(|record| record["event"]["step_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(completion_order, vec!["review_data", "review_api"]);

    let stale = complete(
        &backend,
        31,
        ceremony,
        claims[0].0,
        &claims[0].1["claim_fence"],
    )
    .await;
    assert_eq!(stale["result"]["isError"], true);
    let unchanged = call(
        &backend,
        32,
        "made_get_ceremony_instance",
        json!({"ceremony_id": ceremony}),
    )
    .await;
    assert_eq!(structured(&unchanged), structured(&instance));
}
