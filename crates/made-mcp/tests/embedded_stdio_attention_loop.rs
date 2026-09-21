#![cfg(feature = "embedded")]

//! The integrator loop, end to end, over stdio and a durable store.
//!
//! The ceremony under test is the acceptance ceremony
//! (`tests/e2e/ceremonies/integrator-loop.yaml`): a person delegates
//! once and approves once, and everything in between is the loop's.
//!
//! # How the host is faked
//!
//! `MADE_HOST_ACTIVATION_COMMAND` points at a shell script this test
//! writes. The script saves the envelope and re-enters **this test
//! binary** at [`the_integrator_takes_its_turn`], an ignored test that
//! runs only when the script names it. That turn opens *another*
//! `made-mcp` stdio process against the same store and drives the loop
//! through the ordinary tools: await, acknowledge intent, act,
//! acknowledge processed.
//!
//! Nothing in the turn is a user message. The only two things a person
//! contributes are sealing the `delegate` step — which is what stating
//! the intent means here — and approving the human guard.
//!
//! The second process composes no activation adapter, so a host that
//! works cannot wake itself: the loop ends when there is nothing left
//! it may do, not when a recursion runs out.

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

/// Where the fake host finds the store, the envelopes and the evidence.
const HOME_VAR: &str = "MADE_LOOP_HOME";
/// Where the correlated evidence is written. Defaults inside the test's
/// own scratch directory, so a run that sets nothing leaves nothing
/// behind outside its temporary directory.
const EVIDENCE_VAR: &str = "MADE_LOOP_EVIDENCE_PATH";

/// The name the generated script re-enters this binary by.
const TURN_TEST: &str = "the_integrator_takes_its_turn";

/// Set on the turn when the test drives the ceremony itself. The host
/// still takes what it is owed and closes it; it just does not act, so
/// what moved the session is unambiguously somebody else.
const PASSIVE_VAR: &str = "MADE_LOOP_PASSIVE";

/// Short enough that a killed host's lease is gone by the time its
/// replacement asks, and long enough that the ask itself is not racing
/// its own lease.
const LEASE_MS: u64 = 1_200;

/// Everything the loop's own verbs and the work they drive need.
fn loop_actions() -> Vec<AuthorizationAction> {
    vec![
        AuthorizationAction::StartCeremony,
        AuthorizationAction::StartPublishedCeremony,
        AuthorizationAction::PublishCeremonyDefinition,
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
        AuthorizationAction::DesignAgenticSystem,
        AuthorizationAction::ValidateAgenticSystem,
        AuthorizationAction::PublishAgenticSystem,
        AuthorizationAction::InstantiateAgenticSystem,
        AuthorizationAction::AdvanceAgenticSystemExecution,
        AuthorizationAction::GetAgenticSystemExecution,
    ]
}

// ---------------------------------------------------------------- engine

/// One live `made-mcp` process, spoken to a line at a time.
struct Engine {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Engine {
    /// A process that wakes nobody: the ordinary engine.
    async fn start(home: &Path) -> Self {
        Self::spawn(protected_stdio::command(home, &loop_actions()).await)
    }

    /// A process that wakes the fake host through the command adapter.
    async fn start_waking(home: &Path) -> Self {
        Self::start_waking_a(home, Host::Driving).await
    }

    async fn start_waking_a(home: &Path, host: Host) -> Self {
        let mut command = protected_stdio::command(home, &loop_actions()).await;
        command
            .env(
                made_adapters::activation::COMMAND_ENV,
                activation_script(home, host),
            )
            // The ten-second default is a bound on a real host's turn.
            // This one spawns a compiled binary and an engine, so it is
            // given a minute: a flaky timeout would look like a loop
            // that does not work.
            .env(made_adapters::activation::TIMEOUT_MS_ENV, "60000");
        Self::spawn(command)
    }

    /// The engine the fake host itself talks to, without re-bootstrapping
    /// the authorization policy the parent already opened.
    fn start_inside_the_host(home: &Path) -> Self {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_made-mcp"));
        command
            .env("MADE_MCP_BACKEND", "embedded")
            .env("MADE_MCP_STORE_PATH", store_of(home))
            .env("MADE_AUTH_POLICY_ID", "embedded-stdio-policy")
            .env("MADE_AUTH_TRUSTED_HOST_ID", "embedded-stdio-host")
            .env("MADE_CEREMONY_STORE_ID", "embedded-stdio-test-store")
            .env("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", "a5".repeat(32))
            .env_remove(made_adapters::activation::COMMAND_ENV);
        Self::spawn(command)
    }

    fn spawn(mut command: tokio::process::Command) -> Self {
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
        let line = tokio::time::timeout(std::time::Duration::from_mins(2), self.lines.next_line())
            .await
            .expect("made-mcp answers within two minutes")
            .expect("the pipe stays open")
            .expect("made-mcp answers");
        serde_json::from_str(&line).expect("the answer is one json line")
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

    /// Call one tool and read the refusal it produced.
    async fn refusal(&mut self, tool: &str, arguments: Value) -> String {
        let response = self.call(tool, arguments).await;
        assert_eq!(
            response["result"]["isError"],
            Value::Bool(true),
            "{tool} was supposed to refuse: {response}"
        );
        response["result"].to_string()
    }

    async fn stop(mut self) {
        drop(self.stdin);
        let _ = self.child.wait().await;
    }

    /// End the process the way a crash does, with nothing tidied.
    async fn kill(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

fn store_of(home: &Path) -> PathBuf {
    home.join("ceremonies.sqlite3")
}

fn envelopes_of(home: &Path) -> PathBuf {
    home.join("envelopes")
}

fn evidence_of(home: &Path) -> PathBuf {
    std::env::var(EVIDENCE_VAR).map_or_else(|_| home.join("loop-evidence.jsonl"), PathBuf::from)
}

// ------------------------------------------------------------ fake host

/// Write the script the activation adapter runs, and say where it is.
///
/// Every path is a literal, because the adapter clears the environment:
/// the command gets `MADE_ACTIVATION_*` and nothing else, not even
/// `PATH`, so a script that expected to inherit anything would fail in
/// a way that looks like the loop being broken.
/// What the woken host does with its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    /// Runs the loop: takes the work and moves the session on.
    Driving,
    /// Takes the work and closes it, and moves nothing. What advanced
    /// the session was then somebody else, which is the only way to
    /// show that the bound host is told about a move it did not make.
    Passive,
}

fn activation_script(home: &Path, host: Host) -> PathBuf {
    let path = home.join("wake-the-host.sh");
    let turn_binary = std::env::current_exe().expect("the test binary has a path");
    let body = format!(
        r#"#!/bin/sh
set -e
PATH=/usr/bin:/bin:/usr/local/bin
export PATH
mkdir -p '{envelopes}'
cat > "{envelopes}/$MADE_ACTIVATION_DELIVERY_ID.json"
{home_var}='{home}'
{evidence_var}='{evidence}'
{passive_var}='{passive}'
export {home_var} {evidence_var} {passive_var}
'{turn}' --exact --ignored --nocapture {turn_test} >> '{home}/host-turns.log' 2>&1
echo "woke {{$MADE_ACTIVATION_HOST_KIND}} at $MADE_ACTIVATION_DESTINATION"
"#,
        envelopes = envelopes_of(home).display(),
        home = home.display(),
        home_var = HOME_VAR,
        evidence_var = EVIDENCE_VAR,
        evidence = evidence_of(home).display(),
        passive_var = PASSIVE_VAR,
        passive = u8::from(host == Host::Passive),
        turn = turn_binary.display(),
        turn_test = TURN_TEST,
    );
    write_script(&path, &body);
    path
}

fn write_script(path: &Path, body: &str) {
    let mut file = std::fs::File::create(path).expect("the script is writable");
    file.write_all(body.as_bytes()).expect("the script writes");
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("the script is executable");
    }
}

/// One turn of the integrator, run in its own process by the script.
///
/// Ignored, and named by the script rather than by a test run: there
/// is nothing for it to do unless something woke it.
#[tokio::test]
#[ignore = "re-entered by the activation script, never by a plain test run"]
async fn the_integrator_takes_its_turn() {
    let Ok(home) = std::env::var(HOME_VAR) else {
        return;
    };
    let home = PathBuf::from(home);
    let passive = std::env::var(PASSIVE_VAR).is_ok_and(|raw| raw.trim() == "1");
    let mut host = Engine::start_inside_the_host(&home);
    // Bounded on purpose. A host that kept going while the engine kept
    // answering would hide the difference between a loop that finishes
    // and one that spins.
    for _ in 0..24 {
        let batch = host
            .ok(
                "made_await_integrator_attention",
                json!({
                    "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                    "binding_id": BINDING_ID,
                    "incarnation": INCARNATION,
                    "fence": 0,
                    "limit": 10,
                    "wait_timeout_ms": 100,
                    "lease_duration_ms": 120_000,
                }),
            )
            .await;
        let held = items_of(&batch);
        for item in &held {
            acknowledge(&mut host, item, "intent").await;
            record_evidence(&home, "acknowledged_intent", item, &batch);
        }
        let acted = if passive {
            false
        } else {
            act_once(&mut host, &home).await
        };
        for item in &held {
            acknowledge(&mut host, item, "processed").await;
            record_evidence(&home, "acknowledged_processed", item, &batch);
        }
        if !acted && held.is_empty() {
            break;
        }
    }
    host.stop().await;
}

fn items_of(batch: &Value) -> Vec<Value> {
    batch["items"].as_array().cloned().unwrap_or_default()
}

async fn acknowledge(host: &mut Engine, item: &Value, acknowledgement: &str) {
    let delivery_id = item["delivery_id"].as_str().expect("an offer has an id");
    host.ok(
        "made_acknowledge_integrator_attention",
        json!({
            "binding_id": BINDING_ID,
            "incarnation": INCARNATION,
            "fence": 0,
            "delivery_id": delivery_id,
            "lease_id": item["lease_id"],
            "acknowledgement": acknowledgement,
            "action_kind": "integrated",
            "idempotency_key": format!("loop-{delivery_id}"),
        }),
    )
    .await;
}

/// Move the ceremony on by exactly one step or one transition.
///
/// Returns whether anything moved. The one thing this never does is
/// approve: a loop that could satisfy a human guard would make the
/// guard a formality, and the whole ceremony exists to show that it
/// cannot.
async fn act_once(host: &mut Engine, home: &Path) -> bool {
    let instance = host
        .ok(
            "made_get_ceremony_instance",
            json!({ "ceremony_id": CEREMONY_ID }),
        )
        .await;
    if instance["completed"] == Value::Bool(true) {
        return false;
    }
    if instance["lifecycle"] == "paused" {
        return false;
    }
    let waiting = instance["waiting_for_human"]
        .as_array()
        .is_some_and(|guards| !guards.is_empty());
    if waiting {
        note(home, "the loop stopped in front of a human guard");
        return false;
    }
    if let Some(step) = instance["claimable_step_ids"]
        .as_array()
        .and_then(|steps| steps.first())
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        let round = bump_round(home, &step);
        seal_step(host, &step, output_for(&step, round)).await;
        return true;
    }
    let state = instance["current_state"].as_str().unwrap_or_default();
    let Some(trigger) = trigger_for(state, home) else {
        return false;
    };
    let fired = host
        .call(
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": CEREMONY_ID, "trigger": trigger, "actor_kind": "agent" }),
        )
        .await;
    fired["result"]["isError"] != Value::Bool(true)
}

/// The ceremony's own transition table, as the host knows it.
///
/// A host drives a ceremony it was told about; reading the trigger out
/// of the instance would be reading the engine's own answer back to it.
fn trigger_for(state: &str, home: &Path) -> Option<&'static str> {
    match state {
        "delegate" => Some("hand_over"),
        "implement" => Some("submit"),
        "review" => Some(if rounds(home, "review") > 1 {
            "accept"
        } else {
            "revise"
        }),
        "human_approval" => Some("approve"),
        _ => None,
    }
}

/// What a step seals. The reviewer refuses the first attempt and
/// accepts the second, which is the bounce the ceremony is about.
fn output_for(step: &str, round: u32) -> Value {
    match step {
        "delegate" => json!({ "brief": "close the loop without a second user message" }),
        "implement" => json!({ "patch": format!("attempt {round}") }),
        "review" => json!({
            "accepted": round > 1,
            "note": if round > 1 { "the guard is there" } else { "the guard is missing" },
        }),
        other => json!({ "step": other }),
    }
}

async fn seal_step(host: &mut Engine, step: &str, output: Value) {
    let claim = host
        .ok(
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": CEREMONY_ID,
                "step_id": step,
                "actor_kind": "agent",
                "lease_owner_id": format!("{step}-host"),
                "idempotency_key": format!("{CEREMONY_ID}-{step}-{output}"),
                "lease_ttl_ms": 120_000,
            }),
        )
        .await;
    let mut arguments = json!({
        "ceremony_id": CEREMONY_ID,
        "step_id": step,
        "actor_kind": "agent",
        "status": "completed",
        "output": output,
    });
    if !claim["claim_fence"].is_null() {
        arguments["claim_fence"] = claim["claim_fence"].clone();
    }
    host.ok("made_complete_ceremony_step", arguments).await;
}

/// How many times this step has been sealed, counted in a file so the
/// count survives the turn that wrote it.
fn bump_round(home: &Path, step: &str) -> u32 {
    let next = rounds(home, step) + 1;
    let _ = std::fs::create_dir_all(home.join("rounds"));
    let _ = std::fs::write(home.join("rounds").join(step), next.to_string());
    next
}

fn rounds(home: &Path, step: &str) -> u32 {
    std::fs::read_to_string(home.join("rounds").join(step))
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .unwrap_or(0)
}

fn note(home: &Path, what: &str) {
    append_line(&home.join("host-notes.log"), what);
}

// ------------------------------------------------------------- evidence

/// One line per thing the loop did, threaded by the key that actually
/// threads it.
///
/// The envelope's `correlation_id` is the ceremony's, so every line of
/// a run carries the same one and it correlates nothing. What ties a
/// round together — the knock, the statement of intent, the effect and
/// the acknowledgement that closes it — is the delivery, so that is
/// what the evidence is keyed on. The ceremony's correlation is kept
/// beside it, named for what it is.
fn record_evidence(home: &Path, what: &str, item: &Value, batch: &Value) {
    let delivery_id = item["delivery_id"].as_str().unwrap_or_default();
    let envelope = read_envelope(home, delivery_id);
    let kind = item["attention"]["kind"].as_str().unwrap_or_default();
    let reason = item["attention"]["reason"].as_str().unwrap_or_default();
    let line = json!({
        "what": what,
        // The thread: one delivery, one round, every line of it.
        "correlates_on": delivery_id,
        "delivery_id": delivery_id,
        "attention_id": item["attention"]["attention_id"],
        "source_event_id": item["attention"]["source_event_id"],
        "kind": kind,
        // Two readings share `human_decision_requested`, so the
        // evidence says which of them this was rather than leaving a
        // reader to guess.
        "reading": reading_of(kind, reason),
        "ceremony_id": item["attention"]["ceremony_id"],
        "step_id": item["attention"]["step_id"],
        "reason": reason,
        "loop_state": batch["loop_state"],
        "end_reason": batch["end_reason"],
        "journal_head": batch["journal_head"],
        "ceremony_correlation_id": envelope
            .as_ref()
            .and_then(|envelope| envelope["correlation_id"].clone().into()),
        "woken_by_envelope": envelope.is_some(),
    });
    append_line(&evidence_of(home), &line.to_string());
}

/// Which of a kind's readings a line is, when a kind has more than one.
fn reading_of(kind: &str, reason: &str) -> String {
    if kind != "human_decision_requested" {
        return kind.to_owned();
    }
    if reason.starts_with("a person has ") {
        "human_decision_answered".to_owned()
    } else {
        "human_decision_requested".to_owned()
    }
}

fn read_envelope(home: &Path, delivery_id: &str) -> Option<Value> {
    let raw =
        std::fs::read_to_string(envelopes_of(home).join(format!("{delivery_id}.json"))).ok()?;
    serde_json::from_str(&raw).ok()
}

fn append_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

// ------------------------------------------------------ shared openings

/// Start the acceptance ceremony and put one host in charge of it.
async fn open_the_loop(engine: &mut Engine, activation: &str) {
    // Published rather than mounted inline: a mounted definition lives
    // in the process that mounted it, and this loop is read by a second
    // process and by the process that replaces one that died. The
    // instance then carries the digest it was started from, which is
    // what makes the reading survive both.
    engine
        .ok(
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": CEREMONY_YAML }),
        )
        .await;
    engine
        .ok(
            "made_start_published_ceremony",
            json!({
                "ceremony_id": CEREMONY_ID,
                "ceremony": "integrator_loop",
                "version": "1.0",
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
    seal_step(engine, step, output).await;
}

async fn fire(engine: &mut Engine, trigger: &str) {
    engine
        .ok(
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": CEREMONY_ID, "trigger": trigger, "actor_kind": "agent" }),
        )
        .await;
}

async fn await_once(engine: &mut Engine, wait_ms: u64) -> Value {
    await_leased(engine, wait_ms, 120_000).await
}

async fn await_leased(engine: &mut Engine, wait_ms: u64, lease_ms: u64) -> Value {
    engine
        .ok(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": BINDING_ID,
                "incarnation": INCARNATION,
                "fence": 0,
                "limit": 10,
                "wait_timeout_ms": wait_ms,
                "lease_duration_ms": lease_ms,
            }),
        )
        .await
}

fn kinds_of(batch: &Value) -> Vec<String> {
    items_of(batch)
        .iter()
        .filter_map(|item| item["attention"]["kind"].as_str())
        .map(str::to_owned)
        .collect()
}

// ---------------------------------------------------------------- tests

/// The whole narrative, driven by hand, with nothing waking anybody.
///
/// Proving the ceremony here first means a failure in the activation
/// test is a failure of the activation rather than of the ceremony.
#[tokio::test]
async fn the_acceptance_ceremony_bounces_once_and_stops_at_the_human_guard() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;

    seal(&mut engine, "delegate", json!({ "brief": "do the thing" })).await;
    fire(&mut engine, "hand_over").await;

    seal(
        &mut engine,
        "implement",
        json!({ "patch": "first attempt" }),
    )
    .await;
    fire(&mut engine, "submit").await;
    seal(
        &mut engine,
        "review",
        json!({ "accepted": false, "note": "the guard is missing" }),
    )
    .await;

    let rejected = await_once(&mut engine, 200).await;
    assert!(
        kinds_of(&rejected).contains(&"review_rejected".to_owned()),
        "a refused review has to reach the integrator: {rejected}"
    );

    fire(&mut engine, "revise").await;
    seal(
        &mut engine,
        "implement",
        json!({ "patch": "second attempt" }),
    )
    .await;
    fire(&mut engine, "submit").await;
    seal(
        &mut engine,
        "review",
        json!({ "accepted": true, "note": "the guard is there" }),
    )
    .await;
    fire(&mut engine, "accept").await;

    let waiting = await_once(&mut engine, 200).await;
    assert_eq!(
        waiting["loop_state"], "awaiting_human_decision",
        "the loop must stop in front of the human guard: {waiting}"
    );

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
    let answered = await_once(&mut engine, 200).await;
    assert!(
        kinds_of(&answered).contains(&"human_decision_requested".to_owned()),
        "the answer to a human guard has to reach the loop: {answered}"
    );
    fire(&mut engine, "approve").await;

    let instance = engine
        .ok(
            "made_get_ceremony_instance",
            json!({ "ceremony_id": CEREMONY_ID }),
        )
        .await;
    assert_eq!(instance["completed"], true, "{instance}");
    engine.stop().await;
}

/// The loop, run by a host nobody spoke to after the opening.
#[tokio::test]
async fn a_woken_host_runs_the_loop_and_stops_before_approving() {
    // A named directory when one is asked for, because a failure here
    // happens in a process this one only spawns: the turn's log and the
    // envelopes it was woken with are the only way to read it, and a
    // temporary directory takes them away on the way out.
    let state = tempfile::tempdir().unwrap();
    let home = std::env::var("MADE_LOOP_KEEP_HOME")
        .map(PathBuf::from)
        .inspect(|kept| {
            let _ = std::fs::create_dir_all(kept);
        })
        .unwrap_or_else(|_| state.path().to_path_buf());
    let home = home.as_path();
    let mut engine = Engine::start_waking(home).await;
    open_the_loop(&mut engine, "command").await;

    // The person's first and only instruction: the intent, sealed as
    // the delegating step. The append wakes the host, and everything
    // after this line is the loop's own work.
    seal(
        &mut engine,
        "delegate",
        json!({ "brief": "close the loop" }),
    )
    .await;

    let stopped = await_once(&mut engine, 500).await;
    assert!(
        stopped["items"].as_array().is_some_and(Vec::is_empty),
        "the host took everything it was offered: {stopped}"
    );
    assert_eq!(
        stopped["loop_state"], "awaiting_human_decision",
        "the loop drove itself to the guard and no further: {stopped}"
    );

    let history = read_events(&mut engine).await;
    assert_eq!(
        completed_steps(&history, "implement"),
        2,
        "the refused review has to have sent the work back: {history:?}"
    );
    assert!(
        review_acceptances(&history) == vec![false, true],
        "a refused review must never advance as accepted: {:?}",
        review_acceptances(&history)
    );
    assert!(
        !history
            .iter()
            .any(|event| event["event_type"] == "human_approval_recorded"),
        "no approval may exist before a person gives one"
    );

    // The person's second and last contribution.
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

    let ended = await_once(&mut engine, 1_000).await;
    assert!(
        matches!(
            ended["loop_state"].as_str(),
            Some("completed" | "awaiting_results")
        ),
        "the loop finished the ceremony once the person had answered: {ended}"
    );
    let instance = engine
        .ok(
            "made_get_ceremony_instance",
            json!({ "ceremony_id": CEREMONY_ID }),
        )
        .await;
    assert_eq!(
        instance["completed"], true,
        "the loop closed the ceremony itself: {instance}"
    );

    let approvals = read_events(&mut engine)
        .await
        .into_iter()
        .filter(|event| event["event_type"] == "human_approval_recorded")
        .count();
    assert_eq!(approvals, 1, "exactly one approval, and a person gave it");

    let envelopes = std::fs::read_dir(envelopes_of(home))
        .map(std::iter::Iterator::count)
        .unwrap_or_default();
    assert!(envelopes > 0, "the host was woken by the command adapter");
    let evidence = std::fs::read_to_string(evidence_of(home)).unwrap_or_default();
    assert!(
        evidence.lines().count() >= 2,
        "the turn recorded what it did: {evidence}"
    );
    engine.stop().await;
}

/// A move the integrator did not make still reaches it.
///
/// §8.3's `HumanDecisionRequested`: the session arrives in front of a
/// human guard, and the loop is told so it can say who has to answer.
/// The move is made by the operator here, because a loop told only
/// about its own moves does not need telling at all — and a session
/// any other participant can advance is the ordinary case.
#[tokio::test]
async fn a_move_the_integrator_did_not_make_still_asks_it_for_a_person() {
    let state = tempfile::tempdir().unwrap();
    let home = state.path();
    let mut engine = Engine::start_waking_a(home, Host::Passive).await;
    open_the_loop(&mut engine, "command").await;

    seal(
        &mut engine,
        "delegate",
        json!({ "brief": "somebody else drives" }),
    )
    .await;
    fire(&mut engine, "hand_over").await;
    seal(&mut engine, "implement", json!({ "patch": "only attempt" })).await;
    fire(&mut engine, "submit").await;
    seal(
        &mut engine,
        "review",
        json!({ "accepted": true, "note": "the guard is there" }),
    )
    .await;

    // The operator moves the session in front of the human guard. The
    // integrator did not do this and has no other way of learning it.
    fire(&mut engine, "accept").await;

    let requests = envelopes_of_kind(home, "human_decision_requested");
    assert_eq!(
        requests.len(),
        1,
        "one visit, one guard, one request: {requests:?}"
    );
    assert!(
        requests[0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("human_approved")),
        "the request names the guard a person has to answer: {}",
        requests[0]
    );
    assert!(
        requests[0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("do not answer it yourself")),
        "and says what the loop may not do about it: {}",
        requests[0]
    );

    // The request is the host's to take, not only to be knocked about.
    let owed = await_once(&mut engine, 500).await;
    let approvals = read_events(&mut engine)
        .await
        .into_iter()
        .filter(|event| event["event_type"] == "human_approval_recorded")
        .count();
    assert_eq!(
        approvals, 0,
        "a woken loop reports the guard; it never answers it: {owed}"
    );
    engine.stop().await;
}

/// Every envelope the activation script saved, of one kind.
fn envelopes_of_kind(home: &Path, kind: &str) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(envelopes_of(home)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(|envelope| envelope["kind"] == json!(kind))
        .collect()
}

/// A paused ceremony queues nothing, and says so rather than going
/// quiet.
#[tokio::test]
async fn a_paused_loop_offers_nothing_and_resumes_where_it_stopped() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;
    seal(&mut engine, "delegate", json!({ "brief": "pause me" })).await;

    // Take the offer the delegation produced, so the pause is measured
    // against an empty queue rather than a full one.
    let first = await_once(&mut engine, 200).await;
    for item in items_of(&first) {
        acknowledge(&mut engine, &item, "intent").await;
        acknowledge(&mut engine, &item, "processed").await;
    }

    engine
        .ok(
            "made_pause_ceremony",
            json!({
                "ceremony_id": CEREMONY_ID,
                "actor_id": "acceptance-operator",
                "actor_kind": "service",
                "reason": "the person stepped away",
            }),
        )
        .await;
    let paused = await_once(&mut engine, 200).await;
    assert_eq!(paused["loop_state"], "paused", "{paused}");
    assert!(
        paused["items"].as_array().is_some_and(Vec::is_empty),
        "a paused loop is offered nothing: {paused}"
    );

    engine
        .ok(
            "made_resume_ceremony",
            json!({
                "ceremony_id": CEREMONY_ID,
                "actor_id": "acceptance-operator",
                "actor_kind": "service",
            }),
        )
        .await;
    fire(&mut engine, "hand_over").await;
    let resumed = await_once(&mut engine, 200).await;
    assert_ne!(resumed["loop_state"], "paused", "{resumed}");
    engine.stop().await;
}

/// Three ways of dying, and the same answer each time.
///
/// The cut points are the three the brief names: between the record
/// being sealed and the host being reached, between the host being
/// reached and its statement of intent, and between the effect and the
/// acknowledgement that closes it.
#[tokio::test]
async fn a_killed_process_owes_the_same_work_when_it_comes_back() {
    let state = tempfile::tempdir().unwrap();
    let home = state.path();

    // Cut one: the record is sealed and the process dies before
    // anything is projected or pushed.
    let mut engine = Engine::start(home).await;
    open_the_loop(&mut engine, "none").await;
    seal(&mut engine, "delegate", json!({ "brief": "survive me" })).await;
    engine.kill().await;

    let mut engine = Engine::start(home).await;
    let recovered = await_leased(&mut engine, 500, LEASE_MS).await;
    let delivery = items_of(&recovered)
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("the reopened store still owes the offer: {recovered}"));
    let delivery_id = delivery["delivery_id"].as_str().unwrap().to_owned();

    // Cut two: the host has been handed the work and dies before it
    // says what it means to do. Its lease dies with it, which is what
    // makes the offer somebody else's to take.
    engine.kill().await;
    tokio::time::sleep(std::time::Duration::from_millis(LEASE_MS + 200)).await;
    let mut engine = Engine::start(home).await;
    let again = await_leased(&mut engine, 500, 120_000).await;
    let same = items_of(&again);
    assert_eq!(
        same.first().map(|item| item["delivery_id"].clone()),
        Some(json!(delivery_id)),
        "the same offer comes back, under a new lease: {again}"
    );
    acknowledge(&mut engine, &same[0], "intent").await;

    // The effect. Applying the transition twice is the thing a restart
    // must not do, so it is applied once here and counted afterwards.
    fire(&mut engine, "hand_over").await;

    // Cut three: the effect happened and the process dies before the
    // acknowledgement that would close it.
    engine.kill().await;
    let mut engine = Engine::start(home).await;
    let instance = engine
        .ok(
            "made_get_ceremony_instance",
            json!({ "ceremony_id": CEREMONY_ID }),
        )
        .await;
    assert_eq!(
        instance["current_state"], "implement",
        "the transition happened exactly once: {instance}"
    );
    let transitions = read_events(&mut engine)
        .await
        .into_iter()
        .filter(|event| event["event_type"] == "transition_applied")
        .count();
    assert_eq!(transitions, 1, "the effect was not repeated");

    // And the ledger still says nobody closed it, which is the whole
    // difference between an effect and an acknowledgement.
    let ledger = engine
        .ok(
            "made_list_attention_deliveries",
            json!({ "binding_id": BINDING_ID, "limit": 20 }),
        )
        .await;
    let open = ledger["deliveries"]
        .as_array()
        .expect("the ledger answers with a page")
        .iter()
        .find(|record| record["delivery_id"] == json!(delivery_id))
        .unwrap_or_else(|| panic!("the offer is still on the books: {ledger}"));
    assert_ne!(
        open["state"], "processed",
        "an effect nobody acknowledged is not a closed delivery: {open}"
    );
    engine.stop().await;
}

/// A host that was replaced is never mistaken for the one in charge.
#[tokio::test]
async fn a_stale_incarnation_or_fence_is_never_the_current_integrator() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;
    seal(&mut engine, "delegate", json!({ "brief": "replace me" })).await;

    // The replacement raises the fence and takes the scope.
    let replaced = engine
        .ok(
            "made_bind_ceremony_integrator",
            json!({
                "binding_id": "integrator-loop-binding-2",
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "role_id": "INTEGRATOR",
                "host_kind": "claude-code",
                "address": "integrator-loop-acceptance-session-2",
                "activation": "none",
                "incarnation": "integrator-loop-incarnation-2",
                "replace": true,
                "follow_replacement": true,
            }),
        )
        .await;
    assert_eq!(replaced["outcome"], "replaced", "{replaced}");
    let fence = replaced["binding"]["fence"].as_u64().unwrap();
    assert!(fence > 0, "a replacement raises the fence: {replaced}");

    // The old host asks with its old identity, twice over: the wrong
    // fence, and the wrong incarnation at the right fence.
    let wrong_fence = engine
        .refusal(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": BINDING_ID,
                "incarnation": INCARNATION,
                "fence": 0,
                "wait_timeout_ms": 50,
            }),
        )
        .await;
    assert!(
        wrong_fence.contains("integrator_fence") || wrong_fence.contains("fence"),
        "{wrong_fence}"
    );
    let wrong_incarnation = engine
        .refusal(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": "integrator-loop-binding-2",
                "incarnation": INCARNATION,
                "fence": fence,
                "wait_timeout_ms": 50,
            }),
        )
        .await;
    assert!(
        wrong_incarnation.contains("integrator_fence") || wrong_incarnation.contains("fence"),
        "{wrong_incarnation}"
    );

    // And the replacement, asking properly, is handed the work the
    // replaced host never took.
    let owed = engine
        .ok(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": CEREMONY_ID },
                "binding_id": "integrator-loop-binding-2",
                "incarnation": "integrator-loop-incarnation-2",
                "fence": fence,
                "limit": 10,
                "wait_timeout_ms": 500,
                "lease_duration_ms": 120_000,
            }),
        )
        .await;
    assert!(
        !items_of(&owed).is_empty(),
        "the host in charge is owed the work: {owed}"
    );
    engine.stop().await;
}

/// A loop that keeps asking and is told the same thing every time.
///
/// The lease is an ordinary one. What makes this a stall is that the
/// feed has not moved for this binding and nothing it holds has been
/// closed — which is exactly what a host that took the work and then
/// did nothing looks like, and is not what a host slower than its own
/// lease looks like.
#[tokio::test]
async fn a_loop_that_gets_nowhere_stops_itself() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;
    seal(&mut engine, "delegate", json!({ "brief": "get nowhere" })).await;

    // The first ask moves the head, so it is progress. Every ask after
    // it finds the head where it was left and nothing closed.
    let first = await_leased(&mut engine, 200, 120_000).await;
    assert!(
        !items_of(&first).is_empty(),
        "the first ask is handed the work: {first}"
    );
    let head = first["journal_head"].clone();
    assert!(!head.is_null(), "a projected round has a head: {first}");

    let mut last = first;
    for _ in 0..3 {
        last = await_leased(&mut engine, 100, 120_000).await;
        assert_eq!(
            last["journal_head"], head,
            "nothing was appended, so the head cannot have moved: {last}"
        );
    }
    assert_eq!(
        last["loop_state"], "blocked",
        "three asks at one head with nothing closed is a stall: {last}"
    );
    assert_eq!(
        last["end_reason"], "terminal",
        "a stalled loop is told to stop rather than to come back: {last}"
    );
    engine.stop().await;
}

/// Closing the work clears the count, however long the loop took.
#[tokio::test]
async fn a_slow_host_that_closes_its_work_is_not_stuck() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    open_the_loop(&mut engine, "none").await;
    seal(
        &mut engine,
        "delegate",
        json!({ "brief": "take your time" }),
    )
    .await;

    let first = await_leased(&mut engine, 200, 120_000).await;
    let item = items_of(&first)[0].clone();
    // Two empty asks: the loop is one short of the allowance.
    await_leased(&mut engine, 100, 120_000).await;
    await_leased(&mut engine, 100, 120_000).await;

    acknowledge(&mut engine, &item, "intent").await;
    acknowledge(&mut engine, &item, "processed").await;

    let after = await_leased(&mut engine, 100, 120_000).await;
    assert_ne!(
        after["loop_state"], "blocked",
        "closing the work is progress at an unmoved head: {after}"
    );
    engine.stop().await;
}

// ------------------------------------------------- the ceiling, composed

const SYSTEM_ID: &str = "integrator-loop-system";
const EXECUTION_ID: &str = "integrator-loop-system-run";
const SYSTEM_BINDING_ID: &str = "integrator-loop-system-binding";

/// A composed system whose integrator is allowed exactly one round.
///
/// The ceiling reaches the loop only through a system: a binding made
/// against a single ceremony takes the defaults, and the defaults set
/// no ceiling. This is the path an operator actually has.
fn system_design(max_rounds: u32) -> Value {
    json!({
        "id": SYSTEM_ID,
        "purpose": "show what a loop does once it has used up its rounds",
        "integrator_role_id": "integrator",
        "attention": {
            "kinds": [
                "result_available", "review_rejected", "step_failed", "blocked",
                "human_decision_requested", "deadline_exceeded", "inactivity_detected",
                "intervention_requested", "ceremony_ended",
            ],
            "coalesce_window": 5_000,
            "max_queued": 200,
            "overflow": "drop_oldest_non_blocking",
            "limits": { "max_rounds": max_rounds, "no_progress_rounds": 100 },
        },
        "roles": [
            { "id": "integrator", "responsibility": "drives the loop", "kind": "integrator" },
            { "id": "worker", "responsibility": "does and checks the work", "kind": "contributor" },
        ],
        "participants": [
            { "id": "operator", "role": "integrator", "kind": "person" },
            { "id": "hand", "role": "worker", "kind": "agent" },
        ],
        "topology": [
            { "from": "operator", "to": "hand", "kind": "coordination", "handoff": true },
        ],
        "ceremonies": [
            {
                "id": "loop",
                "pin": { "name": "integrator_loop", "version": "1.0" },
                "purpose": "the acceptance ceremony, run inside a system",
                "activation": { "kind": "manual" },
                "role_bindings": {
                    "INTEGRATOR": "operator",
                    "IMPLEMENTER": "hand",
                    "REVIEWER": "hand",
                    "HUMAN_APPROVER": "operator",
                },
            }
        ],
    })
}

/// Everything before the loop exists: a system, published, running,
/// and the ceremony it opened.
async fn open_a_composed_loop(engine: &mut Engine, max_rounds: u32) -> String {
    engine
        .ok(
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": CEREMONY_YAML }),
        )
        .await;
    engine
        .ok(
            "made_design_agentic_system",
            json!({ "design": system_design(max_rounds) }),
        )
        .await;
    engine
        .ok(
            "made_publish_agentic_system",
            json!({ "system_id": SYSTEM_ID, "revision": 1 }),
        )
        .await;
    engine
        .ok(
            "made_instantiate_agentic_system",
            json!({
                "system_id": SYSTEM_ID,
                "revision": 1,
                "execution_id": EXECUTION_ID,
                "inputs": { "loop": { "intent": "use up the rounds" } },
                "offers": {
                    "operator": { "specialty": "triage", "capabilities": [] },
                    "hand": { "specialty": "triage", "capabilities": [] },
                },
                "actor_id": "acceptance-operator",
                "actor_kind": "service",
            }),
        )
        .await;
    engine
        .ok(
            "made_advance_agentic_system_execution",
            json!({
                "execution_id": EXECUTION_ID,
                "actor_id": "acceptance-operator",
                "actor_kind": "service",
            }),
        )
        .await;
    let execution = engine
        .ok(
            "made_get_agentic_system_execution",
            json!({ "execution_id": EXECUTION_ID }),
        )
        .await;
    engine
        .ok(
            "made_bind_ceremony_integrator",
            json!({
                "binding_id": SYSTEM_BINDING_ID,
                "scope": { "kind": "system_execution", "system_execution_id": EXECUTION_ID },
                "role_id": "INTEGRATOR",
                "host_kind": "claude-code",
                "address": "integrator-loop-system-session",
                "activation": "none",
                "incarnation": INCARNATION,
                "replace": false,
                "follow_replacement": false,
            }),
        )
        .await;
    opened_ceremony(&execution)
}

/// Drive the session from its start to the human guard, by hand.
async fn drive_to_the_guard(engine: &mut Engine, ceremony: &str) {
    seal_in(engine, ceremony, "delegate", json!({ "brief": "too late" })).await;
    fire_in(engine, ceremony, "hand_over").await;
    seal_in(
        engine,
        ceremony,
        "implement",
        json!({ "patch": "too late" }),
    )
    .await;
    fire_in(engine, ceremony, "submit").await;
    seal_in(
        engine,
        ceremony,
        "review",
        json!({ "accepted": true, "note": "too late" }),
    )
    .await;
    fire_in(engine, ceremony, "accept").await;
}

/// A loop that has used up its rounds is told to stop, and from then
/// on hears only what it is not allowed to miss.
#[tokio::test]
async fn a_loop_past_its_ceiling_still_hears_the_things_it_must_not_miss() {
    let state = tempfile::tempdir().unwrap();
    let mut engine = Engine::start(state.path()).await;
    let ceremony = open_a_composed_loop(&mut engine, 1).await;

    // The one round this system allows its integrator.
    let first = await_system(&mut engine, 200).await;
    assert_eq!(
        first["loop_state"], "blocked",
        "one round was the whole allowance: {first}"
    );
    assert_eq!(first["end_reason"], "terminal", "{first}");

    // Everything below is sealed past the ceiling.
    drive_to_the_guard(&mut engine, &ceremony).await;

    let after = await_system(&mut engine, 500).await;
    let kinds = kinds_of(&after);
    assert!(
        !kinds.contains(&"result_available".to_owned()),
        "a loop past its ceiling is offered no more results: {kinds:?}"
    );
    assert!(
        kinds.contains(&"human_decision_requested".to_owned()),
        "but a guard only a person can answer still reaches it: {kinds:?}"
    );

    engine
        .ok(
            "made_approve_ceremony_guard",
            json!({
                "ceremony_id": ceremony,
                "guard_name": "human_approved",
                "role_id": "HUMAN_APPROVER",
                "role_kind": "human",
            }),
        )
        .await;
    fire_in(&mut engine, &ceremony, "approve").await;

    let ended = await_system(&mut engine, 500).await;
    assert!(
        kinds_of(&ended).contains(&"ceremony_ended".to_owned()),
        "and so does the end of the session: {ended}"
    );
    engine.stop().await;
}

/// The ceremony this execution opened, whatever the engine called it.
fn opened_ceremony(execution: &Value) -> String {
    execution["ceremonies"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|entry| entry["instance_id"].as_str())
        .unwrap_or_else(|| panic!("the execution opened a ceremony: {execution}"))
        .to_owned()
}

async fn await_system(engine: &mut Engine, wait_ms: u64) -> Value {
    engine
        .ok(
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "system_execution", "system_execution_id": EXECUTION_ID },
                "binding_id": SYSTEM_BINDING_ID,
                "incarnation": INCARNATION,
                "fence": 0,
                "limit": 20,
                "wait_timeout_ms": wait_ms,
                "lease_duration_ms": 120_000,
            }),
        )
        .await
}

async fn seal_in(engine: &mut Engine, ceremony: &str, step: &str, output: Value) {
    let claim = engine
        .ok(
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": ceremony,
                "step_id": step,
                "actor_kind": "agent",
                "lease_owner_id": format!("{step}-host"),
                "idempotency_key": format!("{ceremony}-{step}-{output}"),
                "lease_ttl_ms": 120_000,
            }),
        )
        .await;
    let mut arguments = json!({
        "ceremony_id": ceremony,
        "step_id": step,
        "actor_kind": "agent",
        "status": "completed",
        "output": output,
    });
    if !claim["claim_fence"].is_null() {
        arguments["claim_fence"] = claim["claim_fence"].clone();
    }
    engine.ok("made_complete_ceremony_step", arguments).await;
}

async fn fire_in(engine: &mut Engine, ceremony: &str, trigger: &str) {
    engine
        .ok(
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": ceremony, "trigger": trigger, "actor_kind": "agent" }),
        )
        .await;
}

// ---------------------------------------------------------------- reads

async fn read_events(engine: &mut Engine) -> Vec<Value> {
    engine
        .ok(
            "made_read_ceremony_events",
            json!({ "ceremony_id": CEREMONY_ID, "limit": 200 }),
        )
        .await["records"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn completed_steps(history: &[Value], step: &str) -> usize {
    history
        .iter()
        .filter(|event| event["event_type"] == "step_completed")
        .filter(|event| event["event"]["step_id"] == json!(step))
        .count()
}

/// Every `accepted` a review sealed, in order.
fn review_acceptances(history: &[Value]) -> Vec<bool> {
    history
        .iter()
        .filter(|event| event["event_type"] == "step_completed")
        .filter(|event| event["event"]["step_id"] == json!("review"))
        .filter_map(|event| {
            let output = &event["event"]["result"]["output"];
            output["attributes"]["accepted"]
                .as_bool()
                .or_else(|| output["accepted"].as_bool())
        })
        .collect()
}
