#![cfg(feature = "embedded")]

//! The integrator loop, end to end, over stdio and a durable store.
//!
//! The ceremony under test is the acceptance ceremony
//! (`tests/e2e/ceremonies/integrator-loop.yaml`): a person states the
//! intent once and approves once, and everything in between is the
//! loop's. What this file proves is that the loop does that work
//! without a user message, that it stops where it must, and that
//! killing the process in the middle of it costs nothing.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use made_core::value_objects::AuthorizationAction;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

#[path = "support/protected_embedded_stdio.rs"]
mod protected_stdio;

/// The acceptance ceremony, read from the file the brief names so the
/// test and the published example cannot drift apart.
const CEREMONY_YAML: &str = include_str!("../../../tests/e2e/ceremonies/integrator-loop.yaml");

const CEREMONY_ID: &str = "integrator-loop-acceptance";
const BINDING_ID: &str = "integrator-loop-binding";
const INCARNATION: &str = "integrator-loop-incarnation-1";

/// Everything the loop's own verbs and the work they drive need.
fn loop_actions() -> Vec<AuthorizationAction> {
    vec![
        AuthorizationAction::StartCeremony,
        AuthorizationAction::GetCeremonyInstance,
        AuthorizationAction::ReadCeremonyEvents,
        AuthorizationAction::ClaimCeremonyStep,
        AuthorizationAction::CompleteCeremonyStep,
        AuthorizationAction::ApplyCeremonyTransition,
        AuthorizationAction::ApproveCeremonyGuard,
        AuthorizationAction::PauseCeremony,
        AuthorizationAction::ResumeCeremony,
        AuthorizationAction::BindCeremonyIntegrator,
        AuthorizationAction::GetCeremonyIntegratorBinding,
        AuthorizationAction::AwaitIntegratorAttention,
        AuthorizationAction::AcknowledgeIntegratorAttention,
        AuthorizationAction::ListAttentionDeliveries,
    ]
}

/// One live `made-mcp` process, spoken to a line at a time.
struct Engine {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Engine {
    async fn start(directory: &Path) -> Self {
        Self::start_with(directory, |_| {}).await
    }

    async fn start_with(directory: &Path, configure: impl FnOnce(&mut tokio::process::Command)) -> Self {
        let mut command = protected_stdio::command(directory, &loop_actions()).await;
        configure(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("made-mcp starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let lines = BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
        Self {
            child,
            stdin,
            lines,
            next_id: 1,
        }
    }

    /// Call one tool and read the answer it produced.
    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        });
        self.stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("the request writes");
        self.stdin.flush().await.expect("the request flushes");
        let line = tokio::time::timeout(std::time::Duration::from_secs(60), self.lines.next_line())
            .await
            .expect("made-mcp answers within a minute")
            .expect("the pipe stays open")
            .expect("made-mcp answers");
        let response: Value = serde_json::from_str(&line).expect("the answer is one json line");
        response
    }

    /// Call one tool and refuse anything but a plain success.
    async fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.call(tool, arguments).await;
        assert_ne!(
            response["result"]["isError"],
            Value::Bool(true),
            "{tool} failed: {response}"
        );
        response["result"]["structuredContent"].clone()
    }

    async fn stop(mut self) {
        drop(self.stdin);
        let _ = self.child.wait().await;
    }

    /// End the process the way a crash does, without a chance to tidy.
    async fn kill(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

fn store_of(directory: &Path) -> PathBuf {
    directory.join("ceremonies.sqlite3")
}

/// Start the acceptance ceremony and put one host in charge of it.
async fn open_the_loop(engine: &mut Engine, activation: &str) {
    engine
        .ok(
            "made_start_ceremony",
            json!({
                "ceremony_id": CEREMONY_ID,
                "definition_yaml": CEREMONY_YAML,
                "actor_id": "acceptance-operator",
                "actor_kind": "service",
                "context": { "intent": "close the loop without a second user message" },
            }),
        )
        .await;
    engine
        .ok(
            "made_bind_ceremony_integrator",
            json!({
                "binding_id": BINDING_ID,
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "role_id": "INTEGRATOR",
                "host_kind": "claude-code",
                "address": "integrator-loop-acceptance-session",
                "activation": activation,
                "incarnation": INCARNATION,
                "replace": false,
                "follow_replacement": false,
            }),
        )
        .await;
}

async fn seal(engine: &mut Engine, step: &str, output: Value) {
    let claim = engine
        .ok(
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": CEREMONY_ID,
                "step_id": step,
                "actor_kind": "agent",
                "lease_owner_id": format!("{step}-host"),
                "idempotency_key": format!("{CEREMONY_ID}-{step}-{}", output),
                "lease_ttl_ms": 60_000,
            }),
        )
        .await;
    let fence = claim["claim_fence"].clone();
    let mut arguments = json!({
        "ceremony_id": CEREMONY_ID,
        "step_id": step,
        "actor_kind": "agent",
        "status": "completed",
        "output": output,
    });
    if !fence.is_null() {
        arguments["claim_fence"] = fence;
    }
    engine.ok("made_complete_ceremony_step", arguments).await;
}

async fn fire(engine: &mut Engine, trigger: &str) {
    engine
        .ok(
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": CEREMONY_ID, "trigger": trigger, "actor_kind": "agent" }),
        )
        .await;
}

/// The whole narrative, driven by hand, with nothing waking anybody.
///
/// This is the shape the fake host has to reproduce on its own, and
/// proving it here first means a failure in the activation test is a
/// failure of the activation rather than of the ceremony.
#[tokio::test]
async fn the_acceptance_ceremony_bounces_once_and_stops_at_the_human_guard() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;

    seal(&mut engine, "delegate", json!({ "brief": "do the thing" })).await;
    fire(&mut engine, "hand_over").await;

    // Round one: the work is done and the review refuses it.
    seal(&mut engine, "implement", json!({ "patch": "first attempt" })).await;
    fire(&mut engine, "submit").await;
    seal(
        &mut engine,
        "review",
        json!({ "accepted": false, "note": "the guard is missing" }),
    )
    .await;

    let rejected = engine
        .ok(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": BINDING_ID,
                "incarnation": INCARNATION,
                "fence": 0,
                "limit": 10,
                "wait_timeout_ms": 200,
                "lease_duration_ms": 60_000,
            }),
        )
        .await;
    let kinds = kinds_of(&rejected);
    assert!(
        kinds.contains(&"review_rejected".to_owned()),
        "a refused review has to reach the integrator: {rejected}"
    );

    fire(&mut engine, "revise").await;

    // Round two: the work is redone and the review accepts it.
    seal(&mut engine, "implement", json!({ "patch": "second attempt" })).await;
    fire(&mut engine, "submit").await;
    seal(
        &mut engine,
        "review",
        json!({ "accepted": true, "note": "the guard is there" }),
    )
    .await;
    fire(&mut engine, "accept").await;

    let waiting = engine
        .ok(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": BINDING_ID,
                "incarnation": INCARNATION,
                "fence": 0,
                "limit": 10,
                "wait_timeout_ms": 200,
                "lease_duration_ms": 60_000,
            }),
        )
        .await;
    assert_eq!(
        waiting["loop_state"], "awaiting_human_decision",
        "the loop must stop in front of the human guard: {waiting}"
    );

    // The approval is the person's, and it is the second and last
    // thing a person contributes.
    engine
        .ok(
            "made_approve_ceremony_guard",
            json!({
                "ceremony_id": CEREMONY_ID,
                "guard_name": "human_approved",
                "role_id": "HUMAN_APPROVER",
                "role_kind": "human",
            }),
        )
        .await;
    fire(&mut engine, "approve").await;

    let instance = engine
        .ok(
            "made_get_ceremony_instance",
            json!({ "ceremony_id": CEREMONY_ID }),
        )
        .await;
    assert_eq!(instance["completed"], true, "{instance}");
    engine.stop().await;
    let _ = store_of(state.path());
}

fn kinds_of(batch: &Value) -> Vec<String> {
    batch["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["attention"]["kind"].as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Keeps the unused-import warning honest while the file grows.
#[allow(dead_code)]
fn write_script(path: &Path, body: &str) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(body.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
