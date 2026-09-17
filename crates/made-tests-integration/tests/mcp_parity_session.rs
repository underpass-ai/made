//! One session, two engines, one answer — shape **and** values.
//!
//! The parity test this replaces compared the shape of a single tool's
//! result and skipped four fields, because the two arms ran different
//! step handlers: the server deliberated through its executor while the
//! in-process edition ran a handler that does nothing. Different
//! handlers mean different values, and different values mean shapes
//! were all that could honestly be compared.
//!
//! Here both arms run the **same** step handler, the same evidence
//! source and the same frozen clock, injected through the fixture and
//! the builder and never through a production default. So the answers
//! are compared field for field, `output`, `details`, `context` and
//! `evidence_pack` included, and the list of paths excused from the
//! comparison is a short const with a reason on every line.
//!
//! Which tools have to be covered is not a list kept here either: it is
//! `docs/architecture/parity.tsv`, the same file F1's set-equality gate
//! reads. A shared tool the script below never calls fails this test by
//! name, so a tool that becomes shared later is covered without anyone
//! editing the comparison (ADR-014, plan §3.6 F4).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use made_adapters::memory::InProcessSessionMemory;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::ParityClock;
use made_tests_integration::parity_evidence_source::ParityEvidenceSource;
use made_tests_integration::parity_step_handler::ParityStepHandler;
use serde_json::{json, Value};

/// The exception list, read at test time from the same file the
/// surface gate reads. Relative to this file, as F1's `include_str!`
/// is relative to its own.
const PARITY_TSV: &str = include_str!("../../../docs/architecture/parity.tsv");

/// The marker a `parity.tsv` cell uses for "this surface does not
/// have it".
const GAP: &str = "-";

/// The one session both engines run.
const SESSION_ID: &str = "parity-session";
/// The session started from a published version.
const PUBLISHED_SESSION_ID: &str = "parity-published-session";
/// The session `made_run_ceremony` opens and finishes in one call.
const ONE_SHOT_ID: &str = "parity-one-shot";
/// The session that decides something inside a shared memory scope.
const MEMORY_FIRST_ID: &str = "parity-memory-first";
/// The session that opens in the same scope afterwards and is told
/// what the first one decided.
const MEMORY_SECOND_ID: &str = "parity-memory-second";
/// The scope both of them declare.
const MEMORY_SCOPE: &str = "team:parity";

/// Values that are allowed to differ, named per tool, with why.
///
/// It was empty until status joined the shared set, and it is still
/// empty for every other tool: with the same step handler, the same
/// evidence source and the same frozen clock on both arms, every
/// field of every shared tool's answer is equal, timestamps included.
/// What keeps it that way is that the script names the things a
/// client can name — the ceremony id, the intervention id, the
/// idempotency key, the lease owner. Left out, each would be minted
/// per engine and land here with a reason.
///
/// An entry is `(tool, path, reason)`. The tool is part of the key
/// because a path excused everywhere is a hole: `.content[].text` is
/// the pretty-printed mirror of `structuredContent`, so excusing it
/// for the one tool whose two answers legitimately differ must not
/// stop it being compared for the other twenty-six. A path is written
/// the way the walk below names one — field names joined by `.`, an
/// array element as `[]` — and every entry carries a one-line reason,
/// which a test asserts.
const NORMALISED: &[(&str, &str, &str)] = &[
    (
        "made_get_status",
        ".structuredContent.version",
        "each arm reports the version of the engine that answered it: the deployed service's \
         over the wire, the made-embedded crate's in process",
    ),
    (
        "made_get_status",
        ".structuredContent.uptime_seconds",
        "uptime is measured from when the engine that answered started — the server process \
         over the wire, the facade's construction in process",
    ),
    (
        "made_get_status",
        ".content[].text",
        "the text block is the pretty-printed mirror of structuredContent, so it carries the \
         version and the uptime verbatim; every other field of the status is still compared \
         in structuredContent",
    ),
];

/// The definition the session runs. Rich on purpose: a step the engine
/// runs, a step a host claims and completes itself, an automated guard,
/// a human guard that is deferred and then approved, two seats, and a
/// table that is asked, answered, evidenced and closed. An empty
/// collection is a collection that cannot disagree.
const PARITY_CEREMONY: &str = r#"
version: "1.0"
name: "parity_session"
states:
  - id: OPEN
    initial: true
  - id: REVIEW
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: REVIEW
    trigger: opened
    guards:
      - work_done
  - from: REVIEW
    to: DONE
    trigger: approve
    guards:
      - human_approved
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
  human_approved:
    type: human
    check: manual_approval
steps:
  - id: work
    state: OPEN
    handler: parity_step
  - id: handoff
    state: REVIEW
    handler: parity_step
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - handoff
      - opened
      - approve
      - request_intervention
      - respond_to_intervention
  - id: OBSERVER
    allowed_actions:
      - respond_to_intervention
"#;

/// The definition that is published, started from its version, and
/// diffed against a later one.
const PUBLISHED_CEREMONY: &str = r#"
version: "1.0"
name: "parity_published"
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
    handler: parity_step
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#;

/// The same ceremony, a materially different graph: something for the
/// diff to report.
const ALTERED_CEREMONY: &str = r#"
version: "1.0"
name: "parity_published"
states:
  - id: OPEN
    initial: true
  - id: CANCELLED
    terminal: true
transitions:
  - from: OPEN
    to: CANCELLED
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: parity_step
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#;

/// A ceremony one call takes from end to end: no guard waits for a
/// person, so `made_run_ceremony` answers with the whole trace.
const ONE_SHOT_CEREMONY: &str = r#"
version: "1.0"
name: "parity_one_shot"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: opened
    guards:
      - work_done
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
steps:
  - id: work
    state: OPEN
    handler: parity_step
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - opened
"#;

/// The intent `made_design_ceremony` is asked to turn into a ceremony.
///
/// Rich for the same reason the definition above is: two stages, a
/// repeat policy whose stop value is not a string, a human gate, a
/// capability that is not a stage, and one number left out so the
/// designer's own default is in the answer that gets compared.
/// Designing reads no store and mints no id, so the two arms have to
/// agree on the whole document.
fn design_intent() -> Value {
    json!({
        "name": "parity_designed",
        "objective": "Choose the lead story and have the editor accept it.",
        "required_inputs": ["brief"],
        "optional_inputs": ["archive"],
        "outputs": ["lead_story"],
        "participants": [
            { "role_id": "WRITER", "capabilities": ["respond_to_intervention"] },
            { "role_id": "EDITOR", "capabilities": ["request_intervention"] }
        ],
        "stages": [
            {
                "id": "draft_options",
                "owner_role_id": "WRITER",
                "instructions": "Draft three candidate leads.",
                "repeat": { "max_iterations": 3, "output_field": "ready", "equals": true }
            },
            {
                "id": "weigh_options",
                "owner_role_id": "EDITOR",
                "instructions": "Weigh them against the brief.",
                "num_agents": 2,
                "review_rounds": 1,
                "see_prior": true
            }
        ],
        "final_approval": { "role_id": "EDITOR" },
        "backoff_seconds": 0
    })
}

// ---------------------------------------------------------------------------
// The two arms
// ---------------------------------------------------------------------------

/// Both engines, wired the same way, each behind the MCP server a
/// client actually talks to.
///
/// Through the server rather than straight into the backend, because
/// that is where the request gate runs and where the JSON-RPC envelope
/// is built: a client meets `tools/call`, not a Rust trait.
struct ParityArms {
    /// Dropping it stops the in-process server.
    _fixture: GrpcFixture,
    /// Dropping it removes the durable store's directory, on the pass
    /// that has one.
    _store_dir: Option<tempfile::TempDir>,
    over_the_wire: MadeMcpServer,
    in_process: MadeMcpServer,
}

impl ParityArms {
    /// The in-process arm over the store a test gets by default.
    async fn start() -> Self {
        Self::over(EmbeddedMade::builder(), None).await
    }

    /// The in-process arm over **the store the local edition ships
    /// with**: WAL-mode SQLite in a directory of its own.
    ///
    /// The session was only ever driven over `InMemoryCeremonyEventStore`
    /// on both arms, and that is not what `made-mcp` opens when somebody
    /// runs it: `EmbeddedMade::open` builds over `SqliteCeremonyStore`.
    /// A gate that compares two in-memory engines proves parity of
    /// something nobody ships (ADR-014).
    ///
    /// Built through the builder rather than through
    /// `EmbeddedMade::open`, and with the same store type `open` uses,
    /// because the session needs the parity handler, evidence source and
    /// clock on both arms — the whole reason its values can be compared
    /// at all — and `open` composes an engine that takes none of them.
    async fn start_on_the_shipped_store() -> Self {
        let directory = tempfile::tempdir().expect("a directory for the durable store");
        let store = Arc::new(
            SqliteCeremonyStore::open(directory.path().join("parity.sqlite3"))
                .expect("the durable SQLite ceremony store should open"),
        );
        Self::over(
            EmbeddedMade::builder()
                .with_ceremony_store(store.clone())
                .with_definition_publications(store),
            Some(directory),
        )
        .await
    }

    async fn over(
        builder: made_embedded::EmbeddedMadeBuilder,
        store_dir: Option<tempfile::TempDir>,
    ) -> Self {
        // One memory per arm, not one between them. Both are
        // in-process and equivalent, so each arm recalls what that arm
        // wrote and the two answers are equal because the engines
        // agree — not because they are reading each other's writes.
        let fixture = GrpcFixture::start_with(
            GrpcFixtureWiring::new()
                .with_step_handler(ParityStepHandler::shared())
                .with_evidence_source(ParityEvidenceSource::shared())
                .with_clock(ParityClock::shared())
                .with_memory(Arc::new(InProcessSessionMemory::new())),
        )
        .await;
        let over_the_wire = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
            format!("http://{}", fixture.addr),
            MadeMcpGrpcTlsConfig::disabled(),
        ));
        let in_process = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(
            builder
                .with_step_handler(ParityStepHandler::shared())
                .with_evidence_source(ParityEvidenceSource::shared())
                .with_clock(ParityClock::shared())
                .with_memory(Arc::new(InProcessSessionMemory::new()))
                .build(),
        ));
        Self {
            _fixture: fixture,
            _store_dir: store_dir,
            over_the_wire,
            in_process,
        }
    }

    /// The same call on both arms, answered as the client sees it.
    async fn call(&self, id: u64, tool: &str, arguments: &Value) -> (Value, Value) {
        (
            call_tool(&self.over_the_wire, id, tool, arguments).await,
            call_tool(&self.in_process, id, tool, arguments).await,
        )
    }
}

/// One `tools/call`, returning the JSON-RPC `result` — the success
/// envelope or the error envelope, whichever the server built.
async fn call_tool(server: &MadeMcpServer, id: u64, tool: &str, arguments: &Value) -> Value {
    let traceparent = deterministic_traceparent(id);
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": arguments,
            "_meta": { "traceparent": traceparent },
        },
    });
    let response = server
        .handle_json_line(&request.to_string())
        .await
        .unwrap_or_else(|| panic!("`{tool}` answered nothing at all"));
    let parsed: Value = serde_json::from_str(&response)
        .unwrap_or_else(|error| panic!("`{tool}` answered something that is not JSON: {error}"));
    parsed.get("result").cloned().unwrap_or_else(|| {
        panic!("`{tool}` answered a JSON-RPC error rather than a result: {parsed}")
    })
}

/// Give both parity arms the same valid W3C context for each scripted call.
/// Production still mints a fresh context when callers omit metadata; focused
/// MCP trace-context tests cover that behavior independently.
fn deterministic_traceparent(id: u64) -> String {
    let non_zero_id = id.max(1);
    format!("00-{non_zero_id:032x}-{non_zero_id:016x}-01")
}

fn structured(result: &Value) -> &Value {
    &result["structuredContent"]
}

fn failed(result: &Value) -> bool {
    result["isError"] == json!(true)
}

// ---------------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------------

/// The scripted session, in order. Every shared tool is in here at
/// least once; the coverage assertion below is what keeps it true.
#[allow(clippy::too_many_lines)] // one entry per call; splitting fragments the session
fn session_script() -> Vec<(&'static str, Value)> {
    vec![
        ("made_design_ceremony", design_intent()),
        (
            "made_validate_ceremony_draft",
            json!({ "definition_yaml": PARITY_CEREMONY }),
        ),
        (
            "made_explain_ceremony_draft",
            json!({ "definition_yaml": PARITY_CEREMONY }),
        ),
        (
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": PUBLISHED_CEREMONY }),
        ),
        (
            "made_diff_ceremony_definitions",
            json!({
                "before": { "ceremony": "parity_published", "version": "1.0" },
                "after": { "definition_yaml": ALTERED_CEREMONY },
            }),
        ),
        (
            "made_start_published_ceremony",
            json!({
                "ceremony": "parity_published",
                "version": "1.0",
                "ceremony_id": PUBLISHED_SESSION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": SESSION_ID,
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "context": { "incident_ref": "INC-42", "severity": 2 },
            }),
        ),
        (
            "made_bind_ceremony_participants",
            json!({
                "ceremony_id": SESSION_ID,
                "seating": { "FACILITATOR": "facilitation" },
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        // The lease owner is named for the same reason the
        // idempotency key is: an omitted one becomes
        // `made-mcp:<backend>` by the rule F2 wrote into the schema,
        // which is the engine's own name and differs by arm on
        // purpose. Reading the stream (F3c) is what first put that
        // value where a comparison could see it.
        (
            "made_run_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parity-work-1",
            }),
        ),
        (
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": SESSION_ID, "trigger": "opened", "actor_kind": "agent" }),
        ),
        // The delegated-host protocol on the step the review state
        // declares: the host takes the lease, does the work where the
        // engine cannot see it, and reports what it saw. The lease
        // owner and the idempotency key are named rather than left to
        // a default, for the reason the header gives: an omitted
        // `lease_owner_id` becomes `made-mcp:<backend>`, which differs
        // by arm on purpose (F2), and an omitted key is minted by the
        // engine.
        (
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "handoff",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parity-handoff-1",
                "lease_ttl_ms": 60_000,
            }),
        ),
        (
            "made_complete_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "handoff",
                "actor_kind": "agent",
                "status": "completed",
                "output": { "handoff_note": "the reviewer has it", "attachments": 2 },
            }),
        ),
        (
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "what-happened",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "opinion",
                "message": "What did you see?",
                // A whole number written with a decimal point and one
                // written without: a `Struct` cannot tell them apart,
                // so before this they were sealed differently by the
                // two engines and the two digests disagreed on an
                // intact chain.
                "details": { "asked_at_state": "REVIEW", "severity": 1.0, "attempt": 1 },
            }),
        ),
        (
            "made_respond_to_ceremony_intervention",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "what-happened",
                "role_id": "OBSERVER",
                "role_kind": "agent",
                "message": "The queue was backing up.",
                "details": {
                    "observed": ["queue_depth", "error_rate"],
                    "confidence": 1.0,
                    "samples": 12,
                },
            }),
        ),
        (
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "inspect-metrics",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "investigation",
                "target_role_ids": ["OBSERVER"],
                "message": "Inspect the checkout metrics.",
            }),
        ),
        (
            "made_collect_ceremony_evidence",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "inspect-metrics",
                "role_id": "OBSERVER",
                "role_kind": "agent",
                "source_id": "observability",
                "query": "Checkout errors over the last five minutes.",
                "details": { "window_minutes": 5 },
            }),
        ),
        (
            "made_assert_ceremony_reason",
            json!({
                "ceremony_id": SESSION_ID,
                "role_id": "OBSERVER",
                "role_kind": "agent",
                "from": { "kind": "contribution", "agenda_item": "inspect-metrics", "ordinal": 0 },
                "to": { "kind": "contribution", "agenda_item": "what-happened", "ordinal": 0 },
                "kind": "chosen_because",
                "why": "The queue growth is what sent me to the metrics.",
                "confidence": "high",
            }),
        ),
        (
            "made_close_ceremony_intervention",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "what-happened",
                "role_id": "FACILITATOR",
                "role_kind": "human",
            }),
        ),
        (
            "made_defer_ceremony_guard",
            json!({
                "ceremony_id": SESSION_ID,
                "guard_name": "human_approved",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "statement": "Not yet.",
                "reason": "The reviewer is out.",
                "reconsider_when": ["the reviewer is back"],
            }),
        ),
        (
            "made_approve_ceremony_guard",
            json!({
                "ceremony_id": SESSION_ID,
                "guard_name": "human_approved",
                "role_id": "FACILITATOR",
                "role_kind": "human",
            }),
        ),
        (
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": SESSION_ID, "trigger": "approve", "actor_kind": "human" }),
        ),
        (
            "made_get_ceremony_instance",
            json!({ "ceremony_id": SESSION_ID }),
        ),
        // The runner is named here for the reason it is named
        // everywhere else in this script: an omitted one becomes
        // `made-mcp:<backend>`, which is the engine's own name and
        // differs by arm on purpose (F2). Reading this stream back is
        // what first put the lease where a comparison could see it.
        (
            "made_run_ceremony",
            json!({
                "ceremony_id": ONE_SHOT_ID,
                "definition_yaml": ONE_SHOT_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "lease_owner_id": "parity-host",
            }),
        ),
        ("made_list_ceremony_instances", json!({})),
        // What the session left behind, read after it is finished so
        // the stream is whole. The sealed records carry their digests
        // and the payload those digests cover, so a client can verify
        // the chain on what it received — and the two arms have to
        // agree on every byte of it, which is only true because the
        // script names the ids and the clock is frozen.
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": SESSION_ID }),
        ),
        // Read again from the middle: a page is a page on both arms,
        // and `next_version` continues the same way.
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": SESSION_ID, "from_version": 2, "limit": 3 }),
        ),
        // The chain over the same records, asked of both arms: the
        // verdict is the engine's own answer to a question the caller
        // could settle from the page above, so the two must agree on
        // the verdict as well as on the records.
        (
            "made_verify_ceremony_journal",
            json!({ "ceremony_id": SESSION_ID }),
        ),
        // And the other two streams this session left behind. One
        // digest compared is one stream proved; the run the engine took
        // end to end and the session bound to a published version are
        // written by different code paths and were never read back.
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": ONE_SHOT_ID }),
        ),
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": PUBLISHED_SESSION_ID }),
        ),
        (
            "made_get_ceremony_transcript",
            json!({ "ceremony_id": SESSION_ID }),
        ),
        // Two sessions in one report, so the order the caller asked
        // for is compared too, and a title so the escaping is. The
        // padding is the point: one arm trimmed the heading and the
        // other did not, so one request rendered two documents.
        (
            "made_generate_ceremony_report",
            json!({
                "ceremony_ids": [SESSION_ID, PUBLISHED_SESSION_ID],
                "title": "  Parity review <both arms>  ",
            }),
        ),
        // How the engine that served all of the above is doing, asked
        // last so the counters it reports are the counters of a
        // session that really ran. `include_stats` is on, because a
        // status whose one open-ended field was never filled in is a
        // status whose one open-ended field was never compared.
        ("made_get_status", json!({ "include_stats": true })),
        ("made_get_metrics", json!({})),
        // E1's gate, driven on both arms. A session that declares a
        // memory scope decides something inside it; a second session
        // that declares the same scope is told so when it opens, and
        // both arms have to say the same thing about what it was told.
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": MEMORY_FIRST_ID,
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "context": { "memory_scope": MEMORY_SCOPE },
            }),
        ),
        (
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": MEMORY_FIRST_ID,
                "intervention_id": "which-rollback",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "opinion",
                "message": "Which rollback do we rehearse?",
            }),
        ),
        // An opinion answered is what the memory projection records as
        // a decision, which is what the second session must be told.
        (
            "made_respond_to_ceremony_intervention",
            json!({
                "ceremony_id": MEMORY_FIRST_ID,
                "intervention_id": "which-rollback",
                "role_id": "OBSERVER",
                "role_kind": "agent",
                "message": "Roll back rather than restart.",
            }),
        ),
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": MEMORY_SECOND_ID,
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "context": { "memory_scope": MEMORY_SCOPE },
            }),
        ),
        (
            "made_get_ceremony_instance",
            json!({ "ceremony_id": MEMORY_SECOND_ID }),
        ),
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": MEMORY_SECOND_ID }),
        ),
    ]
}

#[tokio::test]
async fn one_session_through_every_shared_tool_answers_the_same_on_both_backends() {
    drive_the_whole_session(&ParityArms::start().await).await;
}

/// And again over the store the local edition actually ships with.
///
/// The same script, the same comparison, the same golden document — with
/// the in-process engine writing to WAL-mode SQLite instead of to a map
/// in memory. ADR-014 says the local edition leads and the API keeps
/// parity with it; a gate that only ever compared two in-memory engines
/// was comparing something nobody runs.
#[tokio::test]
async fn the_same_session_answers_the_same_over_the_store_the_edition_ships_with() {
    drive_the_whole_session(&ParityArms::start_on_the_shipped_store().await).await;
}

async fn drive_the_whole_session(arms: &ParityArms) {
    let shared = shared_tools();
    let mut called: BTreeSet<String> = BTreeSet::new();

    for (index, (tool, arguments)) in session_script().into_iter().enumerate() {
        let id = index as u64 + 1;
        let (over_the_wire, in_process) = arms.call(id, tool, &arguments).await;

        assert!(
            !failed(&over_the_wire),
            "`{tool}` failed on the gRPC backend: {over_the_wire:#}"
        );
        assert!(
            !failed(&in_process),
            "`{tool}` failed on the in-process backend: {in_process:#}"
        );
        assert_same_answer(tool, &over_the_wire, &in_process);
        if tool == "made_generate_ceremony_report" {
            assert_the_report_is_the_committed_document(structured(&in_process));
        }
        called.insert((*tool).to_owned());
    }

    // The session really was as rich as it claims: a collection with
    // nothing in it cannot disagree, and two empty answers would agree
    // about nothing at all.
    let session = call_tool(
        &arms.in_process,
        100,
        "made_get_ceremony_instance",
        &json!({ "ceremony_id": SESSION_ID }),
    )
    .await;
    let session = structured(&session);
    assert_eq!(session["current_state"], json!("DONE"), "{session:#}");
    assert_eq!(session["steps"].as_array().map(Vec::len), Some(2));
    assert!(
        !session["steps"][0]["output"]
            .as_object()
            .expect("the step handler answered with an object")
            .is_empty(),
        "the step output is what the old test could not compare; it must not be empty: {session:#}"
    );
    assert_eq!(session["interventions"].as_array().map(Vec::len), Some(2));
    assert_eq!(session["guard_deferrals"].as_array().map(Vec::len), Some(1));
    assert!(session["interventions"]
        .as_array()
        .expect("a session carries its table")
        .iter()
        .any(|item| item["responses"][0]["evidence_pack"].is_object()));

    // Both steps are in the transcript, and one of them is the step
    // the host claimed and completed itself. Until the transcript
    // became a fold of `StepCompleted` (A5) it was a store the two
    // drivers appended to, so the delegated-host protocol left nothing
    // in it and this said one.
    let transcript = call_tool(
        &arms.in_process,
        101,
        "made_get_ceremony_transcript",
        &json!({ "ceremony_id": SESSION_ID }),
    )
    .await;
    let transcript = structured(&transcript);
    assert_eq!(transcript["entry_count"], json!(2), "{transcript:#}");
    assert_eq!(
        transcript["entries"]
            .as_array()
            .expect("a transcript carries its entries")
            .iter()
            .map(|entry| entry["step_id"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["work", "handoff"],
        "{transcript:#}"
    );

    let uncovered: Vec<&str> = shared
        .iter()
        .filter(|tool| !called.contains(*tool))
        .map(String::as_str)
        .collect();
    assert!(
        uncovered.is_empty(),
        "docs/architecture/parity.tsv calls these tools shared and the parity session never \
         calls them: {uncovered:?}. A shared tool nothing drives is a tool whose two answers \
         nobody has compared — add it to `session_script`, or say in the file why it is not \
         shared (ADR-014)."
    );
}

// ---------------------------------------------------------------------------
// The rendered document
// ---------------------------------------------------------------------------

/// The report this session renders, committed.
const PARITY_SESSION_REPORT: &str = include_str!("golden/parity_session_report.md");

/// Set it to rewrite the file from what the session renders. Read the
/// diff before committing it: that is the whole point of the file.
const UPDATE_GOLDEN: &str = "MADE_UPDATE_GOLDEN";

/// Where the golden lives, for the message that tells you how to refresh
/// it.
const GOLDEN_PATH: &str = "crates/made-tests-integration/tests/golden/parity_session_report.md";

/// The report is the serde form of `made-core`'s own entities, fenced as
/// JSON (ADR-006). That makes a `#[serde(rename)]` anywhere in the
/// domain a change to a document operators read — and nothing said so.
/// Both arms render the same bytes, which the comparison above already
/// proves; what it cannot prove is that those bytes are still the ones
/// anybody decided on.
///
/// Committed, so such a change shows up as a diff in review rather than
/// as nothing at all. The session is deterministic — named ids, a frozen
/// clock, a handler whose output is derived from the step — so the
/// document is too.
fn assert_the_report_is_the_committed_document(report: &Value) {
    let rendered = report["report_markdown"]
        .as_str()
        .expect("a report answers with its markdown");

    if std::env::var_os(UPDATE_GOLDEN).is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden/parity_session_report.md");
        std::fs::write(&path, &rendered).expect("the golden document should be writable");
        return;
    }

    assert_eq!(
        rendered, PARITY_SESSION_REPORT,
        "the rendered report is not the committed document ({GOLDEN_PATH}).\n\n\
         A report is the serde form of the entities it quotes, so a rename in \
         `made-core` lands here. If the change is intended, refresh the file with \
         `{UPDATE_GOLDEN}=1 cargo test -p made-tests-integration --test \
         mcp_parity_session` and read the diff."
    );
}

// ---------------------------------------------------------------------------
// Request acceptance
// ---------------------------------------------------------------------------

/// The same call is accepted, or refused with the same envelope, on
/// both arms. It is refused by the server layer before any backend is
/// reached, so the two envelopes are equal byte for byte rather than
/// merely classified the same way.
#[tokio::test]
async fn both_backends_accept_and_refuse_the_same_requests() {
    let arms = ParityArms::start().await;

    for (index, (what, tool, arguments)) in requests_the_gate_refuses().into_iter().enumerate() {
        let (over_the_wire, in_process) = arms.call(index as u64 + 1, tool, &arguments).await;
        for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
            assert!(
                failed(answer),
                "{what} was accepted by the {backend} backend: {answer:#}"
            );
            assert_eq!(
                structured(answer)["code"],
                json!("invalid_request"),
                "{what} on the {backend} backend: {answer:#}"
            );
            assert_eq!(structured(answer)["retryable"], json!(false));
        }
        assert_eq!(
            over_the_wire, in_process,
            "{what} was refused two different ways"
        );
    }

    // And the other direction: a call that leaves out everything it is
    // allowed to leave out, or spells an omission the way a typed host
    // does, is accepted by both.
    for (index, (what, tool, arguments)) in requests_the_gate_accepts().into_iter().enumerate() {
        let (over_the_wire, in_process) = arms.call(index as u64 + 50, tool, &arguments).await;
        for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
            assert!(
                !failed(answer),
                "{what} was refused by the {backend} backend: {answer:#}"
            );
        }
    }
}

/// Every call the published schemas do not admit, with what is wrong
/// with it.
#[allow(clippy::too_many_lines)] // one entry per case; splitting fragments the table
fn requests_the_gate_refuses() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "a required field left out",
            "made_get_ceremony_instance",
            json!({}),
        ),
        (
            "a field of the wrong type",
            "made_get_ceremony_instance",
            json!({ "ceremony_id": 7 }),
        ),
        (
            "a value outside the enum the tool declares",
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": SESSION_ID, "trigger": "opened", "actor_kind": "wizard" }),
        ),
        (
            "an empty ceremony id",
            "made_get_ceremony_instance",
            json!({ "ceremony_id": "" }),
        ),
        (
            "a field the tool does not declare",
            "made_start_ceremony",
            json!({
                "ceremony_id": "typo-session",
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "actor_knid": "service",
            }),
        ),
        ("a tool that does not exist", "made_do_the_thing", json!({})),
        (
            "a report heading that is nothing but space",
            "made_generate_ceremony_report",
            json!({ "ceremony_ids": [SESSION_ID], "title": "   " }),
        ),
        // The three constraints the schemas used to promise in prose
        // only, and the bound every caller-supplied list of ids now
        // declares.
        (
            "both ways of naming a definition at once",
            "made_diff_ceremony_definitions",
            json!({
                "before": { "ceremony": "parity_published", "version": "1.0", "definition_yaml": PARITY_CEREMONY },
                "after": { "definition_yaml": ALTERED_CEREMONY },
            }),
        ),
        (
            "a table with nobody seated at it",
            "made_bind_ceremony_participants",
            json!({
                "ceremony_id": SESSION_ID,
                "seating": {},
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "a failed step that does not say why",
            "made_complete_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "handoff",
                "actor_kind": "agent",
                "status": "failed",
            }),
        ),
        (
            "a report naming more sessions than the bound allows",
            "made_generate_ceremony_report",
            json!({
                "ceremony_ids": (0..101)
                    .map(|index| format!("session-{index}"))
                    .collect::<Vec<_>>(),
            }),
        ),
        (
            "a number no double can count one at a time",
            "made_start_ceremony",
            json!({
                "ceremony_id": "out-of-range-session",
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "context": { "ticket": 1e17 },
            }),
        ),
    ]
}

/// Every call that leaves out what it may leave out, or spells an
/// omission the way a host generated from a typed SDK does.
fn requests_the_gate_accepts() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "a session opened without a context",
            "made_start_ceremony",
            json!({
                "ceremony_id": "optional-fields-session",
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "a step run without a lease owner, an idempotency key or a TTL",
            "made_run_ceremony_step",
            json!({
                "ceremony_id": "optional-fields-session",
                "step_id": "work",
                "actor_kind": "agent",
            }),
        ),
        (
            "an intervention opened without an id, targets or details",
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": "optional-fields-session",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "opinion",
                "message": "Anything to add?",
            }),
        ),
        (
            "a listing asked for with no arguments",
            "made_list_ceremony_instances",
            json!({}),
        ),
        // A host generated from a typed SDK writes an unset optional as
        // `null`, and MCP reserves `_meta` on any object it defines.
        // Both were refused, on both arms, so pointing a client
        // somewhere else did not help it.
        (
            "an intervention whose unset optionals are written as null",
            "made_request_ceremony_intervention",
            json!({
                "ceremony_id": "optional-fields-session",
                "intervention_id": null,
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "opinion",
                "message": "Anything else?",
                "target_role_ids": null,
                "details": null,
            }),
        ),
        (
            "a listing carrying the host's own `_meta`",
            "made_list_ceremony_instances",
            json!({ "_meta": { "progressToken": 7 } }),
        ),
        // A whole number written with a decimal point is read whole on
        // both arms, so it is accepted rather than refused as a
        // non-integer where an integer is declared.
        (
            "a lease length written with a decimal point",
            "made_run_ceremony",
            json!({
                "ceremony_id": "decimal-lease-session",
                "definition_yaml": ONE_SHOT_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "lease_ttl_ms": 60000.0,
            }),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Error envelopes
// ---------------------------------------------------------------------------

/// The same failure, the same envelope. `code` and `retryable` are what
/// a client branches on, and they are compared exactly; the message is
/// the engine's own prose and is only required to carry no transport
/// name.
#[tokio::test]
async fn both_backends_answer_the_same_envelope_for_the_same_failure() {
    let arms = ParityArms::start().await;
    arms.call(
        1,
        "made_start_ceremony",
        &json!({
            "ceremony_id": SESSION_ID,
            "definition_yaml": PARITY_CEREMONY,
            "actor_id": "parity-operator",
            "actor_kind": "service",
        }),
    )
    .await;

    let cases: Vec<(&str, &str, Value, &str)> = vec![
        (
            "a session that is not there",
            "made_get_ceremony_instance",
            json!({ "ceremony_id": "no-such-session" }),
            "not_found",
        ),
        (
            "a transcript of a session that is not there",
            "made_get_ceremony_transcript",
            json!({ "ceremony_id": "no-such-session" }),
            "not_found",
        ),
        (
            "a trigger the current state does not offer",
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": SESSION_ID, "trigger": "approve", "actor_kind": "human" }),
            "refused",
        ),
        (
            "a step outside the current state",
            "made_run_ceremony_step",
            json!({ "ceremony_id": SESSION_ID, "step_id": "nowhere", "actor_kind": "agent" }),
            "refused",
        ),
        (
            "arguments that do not fit the schema",
            "made_get_ceremony_instance",
            json!({}),
            "invalid_request",
        ),
    ];

    for (index, (what, tool, arguments, code)) in cases.into_iter().enumerate() {
        let (over_the_wire, in_process) = arms.call(index as u64 + 10, tool, &arguments).await;
        for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
            assert!(
                failed(answer),
                "{what} succeeded on the {backend} backend: {answer:#}"
            );
            assert_eq!(
                structured(answer)["code"],
                json!(code),
                "{what} on the {backend} backend: {answer:#}"
            );
            assert_eq!(structured(answer)["retryable"], json!(false));
            assert!(
                !structured(answer)["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("gRPC"),
                "the transport reached the envelope on the {backend} backend: {answer:#}"
            );
        }
        assert_eq!(
            structured(&over_the_wire)["code"],
            structured(&in_process)["code"],
            "{what} was classified two different ways"
        );
    }

    // The engine out of reach. The in-process edition has no
    // counterpart by construction — an engine in this process is there
    // or the process is not — so the shape of the envelope is what is
    // asserted, on the arm that can produce it.
    let unreachable = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        "http://127.0.0.1:1",
        MadeMcpGrpcTlsConfig::disabled(),
    ));
    let answer = call_tool(
        &unreachable,
        1,
        "made_get_ceremony_instance",
        &json!({ "ceremony_id": SESSION_ID }),
    )
    .await;
    assert!(failed(&answer), "{answer:#}");
    assert_eq!(structured(&answer)["code"], json!("unavailable"));
    assert_eq!(structured(&answer)["retryable"], json!(true));
    assert!(structured(&answer)["message"].is_string());
}

// ---------------------------------------------------------------------------
// The exception list
// ---------------------------------------------------------------------------

/// Tools `parity.tsv` says both MCP backends serve.
fn shared_tools() -> BTreeSet<String> {
    let mut shared = BTreeSet::new();
    for line in PARITY_TSV.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cells: Vec<&str> = line.split('\t').collect();
        if cells.len() == 6 {
            cells.push("");
        }
        assert_eq!(
            cells.len(),
            7,
            "docs/architecture/parity.tsv has a row of {} columns, expected 6 or 7: {line:?}",
            cells.len()
        );
        if cells[0] == "capability" {
            continue;
        }
        let (over_the_wire, in_process) = (cells[2], cells[3]);
        if over_the_wire == GAP || in_process == GAP {
            continue;
        }
        assert_eq!(
            over_the_wire, in_process,
            "the capability `{}` is served by one tool name over gRPC and another in process; \
             one capability is one tool",
            cells[0]
        );
        shared.insert(over_the_wire.to_owned());
    }
    assert!(
        !shared.is_empty(),
        "docs/architecture/parity.tsv names no shared tool at all"
    );
    shared
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// Two answers to one call, field for field, after normalising the
/// paths listed in [`NORMALISED`].
fn assert_same_answer(tool: &str, over_the_wire: &Value, in_process: &Value) {
    let mut differences = Vec::new();
    compare(
        &normalise(over_the_wire, "", tool),
        &normalise(in_process, "", tool),
        "",
        &mut differences,
    );
    assert!(
        differences.is_empty(),
        "`{tool}` answered differently depending on which engine served it:\n{}\n\
         \n  over the wire: {over_the_wire:#}\n  in process: {in_process:#}",
        differences
            .iter()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Replace the values at this tool's normalised paths with one
/// marker, so the comparison below sees them as equal without seeing
/// them at all. A path excused for one tool stays compared for every
/// other.
fn normalise(value: &Value, path: &str, tool: &str) -> Value {
    if NORMALISED
        .iter()
        .any(|(normalised_tool, normalised, _)| *normalised_tool == tool && *normalised == path)
    {
        return json!("<normalised>");
    }
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, child)| {
                    (
                        key.clone(),
                        normalise(child, &format!("{path}.{key}"), tool),
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalise(item, &format!("{path}[]"), tool))
                .collect(),
        ),
        leaf => leaf.clone(),
    }
}

/// Every path the two answers disagree about, named. Reporting all of
/// them beats reporting the first: a divergence is usually a family.
fn compare(over_the_wire: &Value, in_process: &Value, path: &str, into: &mut Vec<String>) {
    match (over_the_wire, in_process) {
        (Value::Object(wire), Value::Object(process)) => {
            let keys: BTreeSet<&String> = wire.keys().chain(process.keys()).collect();
            for key in keys {
                let child = format!("{path}.{key}");
                match (wire.get(key), process.get(key)) {
                    (Some(wire), Some(process)) => compare(wire, process, &child, into),
                    (Some(wire), None) => {
                        into.push(format!("{child}: only over the wire ({wire})"));
                    }
                    (None, Some(process)) => {
                        into.push(format!("{child}: only in process ({process})"));
                    }
                    (None, None) => unreachable!("the key came from one of the two maps"),
                }
            }
        }
        (Value::Array(wire), Value::Array(process)) => {
            if wire.len() != process.len() {
                into.push(format!(
                    "{path}: {} entries over the wire, {} in process",
                    wire.len(),
                    process.len()
                ));
                return;
            }
            for (index, (wire, process)) in wire.iter().zip(process).enumerate() {
                compare(wire, process, &format!("{path}[{index}]"), into);
            }
        }
        (wire, process) if wire != process => {
            into.push(format!(
                "{path}: {wire} over the wire, {process} in process"
            ));
        }
        _ => {}
    }
}

/// Every normalised path carries a reason, names a tool the session
/// really drives, and is listed once.
#[test]
fn the_normalised_paths_are_declared_once_each_with_a_reason() {
    let driven: BTreeSet<&str> = session_script().into_iter().map(|(tool, _)| tool).collect();
    let mut reasons: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    for (tool, path, reason) in NORMALISED {
        assert!(
            !reason.trim().is_empty(),
            "`{tool}`'s normalised path `{path}` carries no reason; a value excused from the \
             comparison without one is a divergence nobody decided to allow"
        );
        assert!(
            driven.contains(tool),
            "`{path}` is excused for `{tool}`, which the session never calls; an excuse for a \
             tool nobody drives excuses nothing and hides the next one"
        );
        assert!(
            reasons.insert((tool, path), reason).is_none(),
            "`{tool}`'s path `{path}` is normalised twice"
        );
    }
}

/// Both arms really are running the same wiring: the step handler
/// answers, rather than the server deliberating and the in-process
/// edition doing nothing.
#[tokio::test]
async fn both_arms_run_the_same_step_handler() {
    let arms = ParityArms::start().await;
    let (over_the_wire, in_process) = arms
        .call(
            1,
            "made_run_ceremony",
            &json!({
                "ceremony_id": ONE_SHOT_ID,
                "definition_yaml": ONE_SHOT_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        )
        .await;

    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let step = &structured(answer)["steps"][0];
        // A run's trace renders the winner's content, so what proves
        // the handler ran is that content rather than the structured
        // output the session view carries.
        assert_eq!(
            step["output"],
            json!("The parity handler finished `work`."),
            "the {backend} backend ran something other than the parity handler: {answer:#}"
        );
        assert_eq!(step["status"], json!("completed"));
        assert!(
            step["iteration"].is_number(),
            "the {backend} backend must report which turn of the repeat loop a step was"
        );
    }
    assert_same_answer("made_run_ceremony", &over_the_wire, &in_process);

    // And the structured output the session view carries, which is
    // what the old test skipped as open-ended.
    let (over_the_wire, in_process) = arms
        .call(
            2,
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": ONE_SHOT_ID }),
        )
        .await;
    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let output = &structured(answer)["steps"][0]["output"];
        assert_eq!(
            output["handler"],
            json!("parity"),
            "the {backend} backend kept something else as the step's output: {answer:#}"
        );
        assert_eq!(output["step"], json!("work"));
        assert_eq!(output["findings"][0]["verdict"], json!("done"));
    }
    assert_same_answer("made_get_ceremony_instance", &over_the_wire, &in_process);
}

/// E1's gate: what one session decided is what the next session in
/// that scope is told, on both engines.
///
/// The script above already compares the two answers field for field;
/// what this adds is the value. Two arms that agreed on `recollection:
/// null` would pass the comparison and prove nothing, which is the
/// failure the assertion below exists for.
#[tokio::test]
async fn a_session_in_a_shared_scope_is_told_what_the_last_one_decided() {
    let arms = ParityArms::start().await;
    for (index, (tool, arguments)) in session_script().into_iter().enumerate() {
        arms.call(index as u64 + 1, tool, &arguments).await;
    }

    let (over_the_wire, in_process) = arms
        .call(
            200,
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": MEMORY_SECOND_ID }),
        )
        .await;

    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let recollection = &structured(answer)["recollection"];
        assert_eq!(
            recollection["scope"],
            json!(MEMORY_SCOPE),
            "the {backend} backend did not read the declared scope: {answer:#}"
        );
        assert_eq!(recollection["truncated"], json!(false));
        let entries = recollection["entries"]
            .as_array()
            .unwrap_or_else(|| panic!("the {backend} backend answered no entries: {answer:#}"));
        assert!(
            entries.iter().any(|entry| {
                entry["kind"] == json!("decision")
                    && entry["summary"] == json!("Roll back rather than restart.")
                    && entry["from_ceremony_id"] == json!(MEMORY_FIRST_ID)
            }),
            "the {backend} backend did not carry the earlier session's decision: {answer:#}"
        );
    }
    assert_same_answer("made_get_ceremony_instance", &over_the_wire, &in_process);

    // And it is in the stream, right after the opening, so both
    // editions see it by folding rather than by asking memory again.
    let (over_the_wire, in_process) = arms
        .call(
            201,
            "made_read_ceremony_events",
            &json!({ "ceremony_id": MEMORY_SECOND_ID }),
        )
        .await;
    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let records = structured(answer)["records"]
            .as_array()
            .unwrap_or_else(|| panic!("the {backend} backend answered no stream: {answer:#}"));
        assert_eq!(
            records[0]["event_type"],
            json!("ceremony_instance_started"),
            "the {backend} backend: {answer:#}"
        );
        assert_eq!(
            records[1]["event_type"],
            json!("memory_recalled"),
            "the {backend} backend seals the recollection somewhere other than \
             right after the opening: {answer:#}"
        );
        assert_eq!(records[1]["sequence"], json!(2), "{answer:#}");
    }
    assert_same_answer("made_read_ceremony_events", &over_the_wire, &in_process);

    // The other direction, and the reason existing streams are
    // untouched: a session that declares no scope is told nothing and
    // seals nothing beside its opening.
    let (over_the_wire, in_process) = arms
        .call(
            202,
            "made_read_ceremony_events",
            &json!({ "ceremony_id": ONE_SHOT_ID }),
        )
        .await;
    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let records = structured(answer)["records"]
            .as_array()
            .unwrap_or_else(|| panic!("the {backend} backend answered no stream: {answer:#}"));
        assert!(
            !records
                .iter()
                .any(|record| record["event_type"] == json!("memory_recalled")),
            "the {backend} backend recorded a recollection for a session that declared \
             no scope: {answer:#}"
        );
    }
}

/// The clock is frozen and both arms read it, so a timestamp is a
/// compared field rather than a normalised one.
#[tokio::test]
async fn both_arms_read_one_frozen_clock() {
    let arms = ParityArms::start().await;
    let (over_the_wire, in_process) = arms
        .call(
            1,
            "made_start_ceremony",
            &json!({
                "ceremony_id": "clock-parity",
                "definition_yaml": PARITY_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        )
        .await;

    assert_same_answer("made_start_ceremony", &over_the_wire, &in_process);

    // The session view carries no clock of its own, so the instant is
    // read where the session writes one: the table.
    let (over_the_wire, in_process) = arms
        .call(
            2,
            "made_request_ceremony_intervention",
            &json!({
                "ceremony_id": "clock-parity",
                "intervention_id": "when",
                "role_id": "FACILITATOR",
                "role_kind": "human",
                "kind": "opinion",
                "message": "When was this asked?",
            }),
        )
        .await;

    let instant = ParityClock::default().instant();
    for (backend, answer) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        let created_at = structured(answer)["interventions"][0]["created_at"]
            .as_str()
            .unwrap_or_else(|| panic!("the {backend} backend must say when: {answer:#}"));
        assert!(
            created_at.starts_with(&instant.date().to_string()),
            "the {backend} backend did not read the frozen clock: {created_at}"
        );
    }
    assert_same_answer(
        "made_request_ceremony_intervention",
        &over_the_wire,
        &in_process,
    );
}

/// The parity session must stay cheap enough to live in the test job.
/// Two full sessions on two engines, and the budget is wall-clock
/// seconds rather than a feeling.
#[tokio::test]
async fn the_parity_session_costs_seconds_not_minutes() {
    let started = std::time::Instant::now();
    let arms = ParityArms::start().await;
    for (index, (tool, arguments)) in session_script().into_iter().enumerate() {
        arms.call(index as u64 + 1, tool, &arguments).await;
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "the parity session took {elapsed:?}; it runs on every workspace test run"
    );
}
