//! Real `made-mcp` stdio process proof for child orchestration.

#![cfg(feature = "embedded")]

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const CHILD: &str = r#"
version: "1.0"
name: stdio_child
states: [{ id: OPEN, initial: true }, { id: DONE, terminal: true }]
transitions: [{ from: OPEN, to: DONE, trigger: finish, guards: [work_done] }]
steps: [{ id: work, state: OPEN, handler: noop }]
guards:
  work_done: { type: automated, check: "step_status:work:COMPLETED" }
roles: [{ id: CHILD, allowed_actions: [work, finish] }]
"#;

const PARENT: &str = r#"
version: "1.0"
name: stdio_parent
states: [{ id: OPEN, initial: true }, { id: DONE, terminal: true }]
transitions: [{ from: OPEN, to: DONE, trigger: finish, guards: [child_done] }]
steps:
  - id: delegate
    state: OPEN
    handler: must_not_run
    spawn:
      children: [{ ceremony: stdio_child, version: "1.0", inputs: {} }]
      max_children: 1
      max_depth: 2
guards:
  child_done: { type: automated, check: "children_completed:delegate:all" }
roles: [{ id: PARENT, allowed_actions: [delegate, finish] }]
"#;

struct StdioMcp {
    child: Child,
    stdin: ChildStdin,
    stdout: tokio::io::Lines<BufReader<ChildStdout>>,
    id: u64,
}

impl StdioMcp {
    fn start(store: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_made-mcp"))
            .env("MADE_MCP_BACKEND", "embedded")
            .env("MADE_MCP_STORE_PATH", store)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            stdin,
            stdout,
            id: 0,
        }
    }

    async fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.id += 1;
        let request = json!({
            "jsonrpc":"2.0", "id":self.id, "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        });
        self.stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();
        self.stdin.flush().await.unwrap();
        let line = tokio::time::timeout(Duration::from_secs(10), self.stdout.next_line())
            .await
            .expect("stdio response timed out")
            .unwrap()
            .expect("made-mcp closed stdout");
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["result"]["isError"], false, "{name}: {response}");
        response["result"]["structuredContent"].clone()
    }

    async fn stop(mut self) {
        drop(self.stdin);
        let status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .expect("made-mcp did not exit after EOF")
            .unwrap();
        assert!(status.success());
    }
}

#[tokio::test]
async fn embedded_stdio_process_spawns_accepts_recovers_and_reopens_sqlite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let store = directory.path().join("children-stdio.sqlite3");
    let mut mcp = StdioMcp::start(&store);
    for yaml in [CHILD, PARENT] {
        mcp.tool(
            "made_publish_ceremony_definition",
            json!({"definition_yaml":yaml}),
        )
        .await;
    }
    mcp.tool(
        "made_start_published_ceremony",
        json!({
            "ceremony_id":"stdio-parent", "ceremony":"stdio_parent", "version":"1.0",
            "actor_id":"stdio-proof", "actor_kind":"service", "context":{}
        }),
    )
    .await;
    let prepared = mcp
        .tool(
            "made_prepare_ceremony_children",
            json!({
                "ceremony_id":"stdio-parent", "step_id":"delegate", "actor_kind":"agent",
                "lease_owner_id":"stdio-proof", "idempotency_key":"stdio-spawn",
                "lease_ttl_ms":30_000
            }),
        )
        .await;
    let child_id = prepared["child_ids"][0].as_str().unwrap().to_owned();
    mcp.tool(
        "made_run_ceremony_step",
        json!({
            "ceremony_id":child_id, "step_id":"work", "actor_kind":"agent",
            "lease_owner_id":"stdio-proof", "idempotency_key":"stdio-work",
            "lease_ttl_ms":30_000
        }),
    )
    .await;
    mcp.tool(
        "made_apply_ceremony_transition",
        json!({"ceremony_id":child_id,"trigger":"finish","actor_kind":"agent"}),
    )
    .await;
    let history = mcp
        .tool(
            "made_read_ceremony_events",
            json!({"ceremony_id":child_id,"from_version":0,"limit":200}),
        )
        .await;
    let terminal = history["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["event_type"] == "ceremony_completed")
        .unwrap()["event_id"]
        .as_str()
        .unwrap()
        .to_owned();
    mcp.tool(
        "made_accept_child_completion",
        json!({"child_id":child_id,"terminal_event_id":terminal}),
    )
    .await;
    mcp.tool("made_recover_ceremony_children", json!({"limit":1000}))
        .await;
    let completed = mcp
        .tool(
            "made_apply_ceremony_transition",
            json!({"ceremony_id":"stdio-parent","trigger":"finish","actor_kind":"agent"}),
        )
        .await;
    assert_eq!(completed["completed"], true);
    mcp.stop().await;

    let mut reopened = StdioMcp::start(&store);
    let instance = reopened
        .tool(
            "made_get_ceremony_instance",
            json!({"ceremony_id":"stdio-parent"}),
        )
        .await;
    assert_eq!(instance["completed"], true);
    assert_eq!(
        instance["child_groups"][0]["completions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    reopened.stop().await;
}
