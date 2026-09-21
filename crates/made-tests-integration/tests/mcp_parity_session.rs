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

use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::memory::{
    InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore, InProcessSessionMemory,
};
use made_adapters::noop::NoopExecutor;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::validators::{
    AllowedStringValuesValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    ContinueAcceptedCeremonyWorkUseCase, ContinueAcceptedStepClaimUseCase,
    ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate,
};
use made_app::services::{CeremonyEventFanout, SessionStream};
use made_app::usecases::{
    CeremonySearchCursorCodec, CeremonySearchCursorKey, CeremonySearchCursorNamespace,
};
use made_core::ports::{
    AuthorizationPolicyStorePort, CeremonyEventStorePort, CeremonySnapshotStorePort, ClockPort,
    ValidatorPort,
};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationScope, CeremonyId, DelegationDepth, ExecutionOperationId, PrincipalId,
    PrincipalKind, SeparationRule, StateIteration, StateVisit, StepId, StepIteration,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::{MadeMcpGrpcTlsConfig, ToolTraceContext};
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::ParityClock;
use made_tests_integration::parity_evidence_source::ParityEvidenceSource;
use made_tests_integration::parity_step_handler::ParityStepHandler;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[path = "mcp_parity_session/agentic_system.rs"]
mod agentic_system;
#[path = "mcp_parity_session/council_journal.rs"]
mod council_journal;
#[path = "mcp_parity_session/dynamic_roles.rs"]
mod dynamic_roles;
#[path = "mcp_parity_session/execution_receipts.rs"]
mod execution_receipts;
#[path = "mcp_parity_session/host_handoff.rs"]
mod host_handoff;
#[path = "mcp_parity_session/integrator_loop.rs"]
mod integrator_loop;
#[path = "mcp_parity_session/intervention_delivery.rs"]
mod intervention_delivery;
#[path = "mcp_parity_session/optionals.rs"]
mod optionals;
#[path = "mcp_parity_session/state_repeat.rs"]
mod state_repeat;
#[path = "mcp_parity_session/state_visits.rs"]
mod state_visits;
#[path = "mcp_parity_session/succession.rs"]
mod succession;

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
/// A distinct published session whose durable root budget the parity run exercises.
const BUDGET_SESSION_ID: &str = "parity-budget-session";
/// A post-report session that keeps the live-agent parity calls out of the
/// committed report fixture while still exercising a real accepted claim.
const AGENT_STATUS_SESSION_ID: &str = "parity-agent-status-session";
/// The session `made_run_ceremony` opens and finishes in one call.
const ONE_SHOT_ID: &str = "parity-one-shot";
/// The session that decides something inside a shared memory scope.
const MEMORY_FIRST_ID: &str = "parity-memory-first";
/// The session that opens in the same scope afterwards and is told
/// what the first one decided.
const MEMORY_SECOND_ID: &str = "parity-memory-second";
/// The scope both of them declare.
const MEMORY_SCOPE: &str = "team:parity";
const CONCURRENT_SESSION_ID: &str = "parity-concurrent";
const CHILD_PARENT_ID: &str = "parity-child-parent";
const CHILD_PLACEHOLDER: &str = "$parity-child-0";
const TERMINAL_PLACEHOLDER: &str = "$parity-child-terminal";
const ARTIFACT_UPLOAD_PLACEHOLDER: &str = "$parity-artifact-upload";
const ABORT_UPLOAD_PLACEHOLDER: &str = "$parity-abort-upload";

/// Values that are allowed to differ, named per tool, with why.
///
/// The shared handler, evidence source and frozen clock make domain
/// output and timestamps deterministic. Independently minted identities
/// and wall-clock measurements still differ between executions. Only
/// those exact paths are listed below; their surrounding results remain
/// compared. The script explicitly supplies every identity clients can
/// name, including ceremony, intervention, idempotency key and lease owner.
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
    // A lease is minted per engine, like a council lease and an upload:
    // the two arms must agree on what was handed over and to whom, not
    // on the opaque ticket each one minted to hand it over with.
    (
        "made_pull_ceremony_agent_interventions",
        ".structuredContent.items[].lease_id",
        "each ledger mints its own opaque exclusive delivery lease identity",
    ),
    (
        "made_pull_ceremony_agent_interventions",
        ".content[].text.items[].lease_id",
        "the text projection mirrors the independently minted delivery lease identity",
    ),
    (
        "made_pull_ceremony_agent_interventions",
        ".structuredContent.items[].intervention.routes[].lease_id",
        "the route carries the same independently minted lease identity",
    ),
    (
        "made_pull_ceremony_agent_interventions",
        ".content[].text.items[].intervention.routes[].lease_id",
        "the text projection mirrors the route's minted lease identity",
    ),
    ("made_lease_council_events", ".structuredContent.lease.id", "each independent council store mints an opaque exclusive lease identity"),
    (
        "made_await_integrator_attention",
        ".structuredContent.items[].lease_id",
        "each ledger mints its own opaque exclusive attention lease identity",
    ),
    (
        "made_await_integrator_attention",
        ".content[].text.items[].lease_id",
        "the text projection mirrors the independently minted attention lease identity",
    ),
    (
        "made_acknowledge_integrator_attention",
        ".structuredContent.delivery.lease_id",
        "the acknowledged delivery carries the lease its own arm minted",
    ),
    (
        "made_acknowledge_integrator_attention",
        ".content[].text.delivery.lease_id",
        "the text projection mirrors the lease its own arm minted",
    ),
    (
        "made_list_attention_deliveries",
        ".structuredContent.deliveries[].lease_id",
        "the loop's paperwork carries the lease each ledger minted",
    ),
    (
        "made_list_attention_deliveries",
        ".content[].text.deliveries[].lease_id",
        "the text projection mirrors the lease each ledger minted",
    ),
    ("made_lease_council_events", ".content[].text.lease.id", "the text projection mirrors the independently minted council lease identity"),
    (
        "made_begin_artifact_upload",
        ".structuredContent.upload_id",
        "each isolated artifact store mints its own opaque upload identity",
    ),
    (
        "made_begin_artifact_upload",
        ".content[].text.upload_id",
        "the text projection mirrors the independently minted upload identity",
    ),
    (
        "made_put_artifact_chunk",
        ".structuredContent.upload_id",
        "chunk progress retains the opaque upload identity minted by each store",
    ),
    (
        "made_put_artifact_chunk",
        ".content[].text.upload_id",
        "the text projection mirrors the upload identity retained by each store",
    ),
    (
        "made_deliberate",
        ".structuredContent.winner_proposal_id",
        "proposal ids are minted independently by each engine",
    ),
    (
        "made_deliberate",
        ".structuredContent.results[].proposal.proposal_id",
        "proposal ids are minted independently by each engine",
    ),
    (
        "made_deliberate",
        ".content[].text.winner_proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_deliberate",
        ".content[].text.results[].proposal.proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_stream_deliberation",
        ".structuredContent.frames[].payload.result.proposal.proposal_id",
        "the terminal stream frame carries an independently minted proposal id",
    ),
    (
        "made_stream_deliberation",
        ".structuredContent.winner.proposal.proposal_id",
        "the collected winner carries an independently minted proposal id",
    ),
    (
        "made_stream_deliberation",
        ".content[].text.frames[].payload.result.proposal.proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_stream_deliberation",
        ".content[].text.winner.proposal.proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_get_deliberation_result",
        ".structuredContent.result.winner_proposal_id",
        "the stored deliberation retains the proposal id minted by its engine",
    ),
    (
        "made_get_deliberation_result",
        ".structuredContent.result.results[].proposal.proposal_id",
        "the stored deliberation retains the proposal id minted by its engine",
    ),
    (
        "made_get_deliberation_result",
        ".content[].text.result.winner_proposal_id",
        "the text projection mirrors the proposal id stored by its engine",
    ),
    (
        "made_get_deliberation_result",
        ".content[].text.result.results[].proposal.proposal_id",
        "the text projection mirrors the proposal id stored by its engine",
    ),
    (
        "made_orchestrate",
        ".structuredContent.execution_id",
        "the injected executor mints one execution id per engine",
    ),
    (
        "made_orchestrate",
        ".structuredContent.winner.proposal.proposal_id",
        "proposal ids are minted independently by each engine",
    ),
    (
        "made_orchestrate",
        ".content[].text.execution_id",
        "the text projection mirrors the independently minted execution id",
    ),
    (
        "made_orchestrate",
        ".content[].text.winner.proposal.proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_process_trigger_event",
        ".structuredContent.ack.dispatched_task_ids[]",
        "auto-dispatch mints one task id per engine",
    ),
    (
        "made_process_trigger_event",
        ".content[].text.ack.dispatched_task_ids[]",
        "the text projection mirrors the independently minted task id",
    ),
    (
        "made_run_council_decision",
        ".structuredContent.duration_ms",
        "the use case measures each execution with std::time::Instant, outside the frozen domain clock",
    ),
    (
        "made_run_council_decision",
        ".content[].text.duration_ms",
        "the text projection mirrors the independently measured elapsed duration",
    ),
    (
        "made_run_council_decision",
        ".structuredContent.task_id",
        "the decision use case mints one task id per engine",
    ),
    (
        "made_run_council_decision",
        ".structuredContent.winner.proposal.proposal_id",
        "the decision proposal id is minted independently by each engine",
    ),
    (
        "made_run_council_decision",
        ".structuredContent.candidates[].proposal_id",
        "the candidate repeats the independently minted proposal id",
    ),
    (
        "made_run_council_decision",
        ".content[].text.task_id",
        "the text projection mirrors the independently minted task id",
    ),
    (
        "made_run_council_decision",
        ".content[].text.winner.proposal.proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_run_council_decision",
        ".content[].text.candidates[].proposal_id",
        "the text projection mirrors the independently minted proposal id",
    ),
    (
        "made_get_status",
        ".structuredContent.version",
        "each arm reports the version of the engine that answered it: the deployed service's \
         over the wire, the made-embedded crate's in process",
    ),
    (
        "made_get_status",
        ".content[].text.version",
        "the JSON text mirrors structuredContent; only the engine version differs, while \
         uptime, health and statistics remain compared inside both representations",
    ),
    (
        "made_get_metrics",
        ".structuredContent.registry_text",
        "Prometheus text exposition is a transport representation whose family and sample values \
         are compared exactly through the structured registry projection",
    ),
    (
        "made_get_metrics",
        ".content[].text.registry_text",
        "the metrics text block mirrors structuredContent as JSON, so only its nested Prometheus \
         exposition is normalised while stats and the structured registry remain compared",
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
    context_writes:
      last_step: step
  - id: handoff
    state: REVIEW
    handler: parity_step
    role_from: context.next_role
    allowed_roles: [FACILITATOR]
    context_writes:
      final_summary: handoff_note
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

const CONCURRENT_CEREMONY: &str = r#"
version: "1.0"
name: "parity_concurrent"
max_parallel: 2
states:
  - id: OPEN
    initial: true
    execution: concurrent
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards:
      - one_done
guards:
  one_done:
    type: automated
    check: any_step_completed
steps:
  - id: a
    state: OPEN
    handler: parity_step
  - id: b
    state: OPEN
    handler: parity_step
  - id: c
    state: OPEN
    handler: parity_step
roles:
  - id: A
    allowed_actions: [a, finish]
  - id: B
    allowed_actions: [b]
  - id: C
    allowed_actions: [c]
"#;

const CHILD_CEREMONY: &str = r#"
version: "1.0"
name: "parity_child"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [work_done]
steps:
  - id: work
    state: OPEN
    handler: parity_step
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: CHILD
    allowed_actions: [work, finish]
"#;

const CHILD_PARENT_CEREMONY: &str = r#"
version: "1.0"
name: "parity_child_parent"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [child_done]
steps:
  - id: delegate
    state: OPEN
    handler: must_not_run
    spawn:
      children:
        - ceremony: parity_child
          version: "1.0"
          inputs: {}
      max_children: 1
      max_depth: 2
guards:
  child_done:
    type: automated
    check: "children_completed:delegate:all"
roles:
  - id: PARENT
    allowed_actions: [delegate, finish]
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
        "backoff_seconds": 0,
        "max_transitions": 12,
        "max_bounces": 3
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
    fixture: GrpcFixture,
    /// Dropping it removes the durable store's directory, on the pass
    /// that has one.
    _store_dir: Option<tempfile::TempDir>,
    /// The two artifact roots must outlive their independently composed stores.
    _artifact_store_dirs: Vec<tempfile::TempDir>,
    over_the_wire: MadeMcpServer,
    in_process: MadeMcpServer,
    claims: std::sync::Mutex<std::collections::BTreeMap<(String, String), Value>>,
    children: std::sync::Mutex<BTreeMap<String, Vec<String>>>,
    terminals: std::sync::Mutex<BTreeMap<String, String>>,
    uploads: std::sync::Mutex<BTreeMap<String, (String, String)>>,
    council_leases: std::sync::Mutex<BTreeMap<String, (Value, Value)>>,
    /// What each arm's pull handed out, so the acknowledgement that
    /// follows can present the ticket that arm actually issued. A lease
    /// id is minted per engine; scripting one literal would compare an
    /// acknowledgement against a refusal.
    intervention_leases: std::sync::Mutex<BTreeMap<String, [(String, String); 2]>>,
    receipt_stores: Vec<Arc<dyn made_core::ports::ExecutionReceiptStorePort>>,
    receipt_artifacts: Vec<Arc<dyn made_core::ports::ArtifactStorePort>>,
    opaque_authorization_targets: std::sync::Mutex<BTreeMap<String, [(String, String); 2]>>,
    artifact_authorizations: std::sync::Mutex<Vec<[Value; 2]>>,
}

fn parity_council_validators() -> Vec<Arc<dyn ValidatorPort>> {
    vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
        Arc::new(JsonSchemaValidator::new()),
    ]
}

struct ParityEmbeddedAuthorization {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    gate: TrustedHostAuthorizationGate,
    continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

async fn parity_embedded_authorization(clock: Arc<dyn ClockPort>) -> ParityEmbeddedAuthorization {
    let policy_id = AuthorizationPolicyId::new("grpc-fixture").unwrap();
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("grpc-fixture-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    let store: Arc<dyn AuthorizationPolicyStorePort> =
        Arc::new(InMemoryAuthorizationPolicyStore::new());
    let administration = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    administration
        .open(
            principal.clone(),
            vec![SeparationRule::new(
                AuthorizationAction::ApproveCeremonyGuard,
                AuthorizationAction::MountDefinition,
            )
            .unwrap()],
        )
        .await
        .unwrap();
    let (administration_actions, business_actions): (Vec<_>, Vec<_>) =
        parity_authorization_actions()
            .into_iter()
            .partition(|action| is_administration_action(*action));
    for (grant_id, actions) in [
        ("grpc-fixture-business-actions", business_actions),
        ("grpc-fixture-authorization-admin", administration_actions),
    ] {
        administration
            .issue(
                &principal,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new(grant_id).unwrap(),
                    principal.id().clone(),
                    actions,
                    AuthorizationScope::Global,
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(principal.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let ttl = AuthorizationDecisionTtl::from_seconds(60).unwrap();
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        ttl,
    ));
    let gate = TrustedHostAuthorizationGate::new(authorize, principal).unwrap();
    let continuation = Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock,
        ttl,
    ));
    ParityEmbeddedAuthorization {
        policy_id,
        store,
        gate,
        continuation,
    }
}

fn parity_authorization_actions() -> Vec<AuthorizationAction> {
    let mut actions: Vec<_> = shared_tools()
        .into_iter()
        .map(|tool| match tool.as_str() {
            "made_get_budget_report" | "made_list_pending_budget_reservations" => {
                AuthorizationAction::ReadBudget
            }
            "made_get_authorization_policy" => AuthorizationAction::ReadAuthorizationPolicy,
            "made_issue_authorization_grant" => AuthorizationAction::IssueAuthorizationGrant,
            "made_revoke_authorization_grant" => AuthorizationAction::RevokeAuthorizationGrant,
            "made_list_authorization_decisions" => AuthorizationAction::ReadAuthorizationDecisions,
            "made_approve_authorization_operation" => AuthorizationAction::ApproveCeremonyGuard,
            name => serde_json::from_value(json!(name
                .strip_prefix("made_")
                .expect("every shared MCP tool carries the made_ prefix")))
            .unwrap_or_else(|_| panic!("shared MCP tool `{name}` has no authorization action")),
        })
        .collect();
    // The typed gRPC fixture also serves direct-RPC integration tests. Keep
    // the policy observed through its public admin RPC byte-for-byte equal on
    // the embedded parity arm, including capabilities without a shared MCP
    // tool. These do not count as covered MCP calls below.
    actions.extend([
        AuthorizationAction::GetCeremonyDefinition,
        AuthorizationAction::ListCeremonyDefinitions,
        AuthorizationAction::MountDefinition,
        AuthorizationAction::ReserveBudget,
        AuthorizationAction::ReconcileBudget,
    ]);
    actions.sort_unstable();
    actions.dedup();
    actions
}

fn is_administration_action(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::ReadAuthorizationPolicy
            | AuthorizationAction::IssueAuthorizationGrant
            | AuthorizationAction::RevokeAuthorizationGrant
            | AuthorizationAction::ReadAuthorizationDecisions
    )
}

fn parity_session_stream(
    events: Arc<dyn CeremonyEventStorePort>,
    snapshots: Arc<dyn CeremonySnapshotStorePort>,
) -> Arc<SessionStream> {
    Arc::new(SessionStream::new_authorized(
        events,
        snapshots,
        Arc::new(CeremonyEventFanout::new(Vec::new())),
    ))
}

impl ParityArms {
    /// The in-process arm over the store a test gets by default.
    async fn start() -> Self {
        let store = Arc::new(InMemoryCeremonyEventStore::new());
        let stream = parity_session_stream(store.clone(), store.clone());
        Self::over(
            EmbeddedMade::builder().with_ceremony_store(store),
            None,
            stream,
        )
        .await
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
        let directory = council_journal::scratch_directory("ceremony");
        let store = Arc::new(
            SqliteCeremonyStore::open(directory.path().join("parity.sqlite3"))
                .expect("the durable SQLite ceremony store should open"),
        );
        let stream = parity_session_stream(store.clone(), store.clone());
        Self::over(
            EmbeddedMade::builder()
                .with_ceremony_store(store.clone())
                .with_definition_publications(store),
            Some(directory),
            stream,
        )
        .await
    }

    async fn over(
        builder: made_embedded::EmbeddedMadeBuilder,
        store_dir: Option<tempfile::TempDir>,
        ceremony_stream: Arc<SessionStream>,
    ) -> Self {
        // One memory per arm, not one between them. Both are
        // in-process and equivalent, so each arm recalls what that arm
        // wrote and the two answers are equal because the engines
        // agree — not because they are reading each other's writes.
        let wire_artifact_dir = council_journal::scratch_directory("wire-artifacts");
        let local_artifact_dir = council_journal::scratch_directory("local-artifacts");
        let wire_artifacts = Arc::new(
            LocalArtifactStore::open(wire_artifact_dir.path())
                .expect("the gRPC artifact store should open"),
        );
        let local_artifacts = Arc::new(
            LocalArtifactStore::open(local_artifact_dir.path())
                .expect("the embedded artifact store should open"),
        );
        let wire_journal = council_journal::seeded(wire_artifact_dir.path()).await;
        let local_journal = council_journal::seeded(local_artifact_dir.path()).await;
        let wire_receipts: Arc<dyn made_core::ports::ExecutionReceiptStorePort> = Arc::new(
            SqliteCeremonyStore::open(wire_artifact_dir.path().join("receipts.sqlite3")).unwrap(),
        );
        let local_receipts: Arc<dyn made_core::ports::ExecutionReceiptStorePort> = Arc::new(
            SqliteCeremonyStore::open(local_artifact_dir.path().join("receipts.sqlite3")).unwrap(),
        );
        let fixture = GrpcFixture::start_with(
            GrpcFixtureWiring::new()
                .with_step_handler(ParityStepHandler::shared())
                .with_evidence_source(ParityEvidenceSource::shared())
                .with_clock(ParityClock::shared())
                .with_memory(Arc::new(InProcessSessionMemory::new()))
                .with_artifact_store(wire_artifacts.clone())
                .with_council_journal(wire_journal)
                .with_execution_receipts(wire_receipts.clone()),
        )
        .await;
        let embedded_authorization = parity_embedded_authorization(ParityClock::shared()).await;
        let over_the_wire = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
            format!("http://{}", fixture.addr),
            MadeMcpGrpcTlsConfig::disabled(),
        ));
        let embedded_gate = embedded_authorization.gate.clone();
        let embedded_policy_id = embedded_authorization.policy_id.clone();
        let embedded_policy_store = embedded_authorization.store.clone();
        let embedded_step_continuation = Arc::new(ContinueAcceptedStepClaimUseCase::new(
            ceremony_stream,
            embedded_authorization.continuation.clone(),
            ParityClock::shared(),
        ));
        let made = builder
            .with_step_handler(ParityStepHandler::shared())
            .with_evidence_source(ParityEvidenceSource::shared())
            .with_clock(ParityClock::shared())
            .with_ceremony_search_cursors(CeremonySearchCursorCodec::new(
                CeremonySearchCursorKey::new([0x5a; 32]),
                CeremonySearchCursorNamespace::new("grpc-fixture-store", "grpc-fixture").unwrap(),
            ))
            .with_authorization(Arc::new(embedded_gate.clone()))
            .with_memory(Arc::new(InProcessSessionMemory::new()))
            .with_council_validators(parity_council_validators())
            .with_executor(Arc::new(NoopExecutor::new()))
            .with_artifact_store(local_artifacts.clone())
            .with_council_journal(local_journal)
            .with_execution_receipt_store(local_receipts.clone())
            .build()
            .with_authorization_policy(embedded_policy_id.clone(), embedded_policy_store.clone());
        let in_process = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::with_authorization(
            made,
            embedded_gate,
            ReadAuthorizationPolicyUseCase::new(embedded_policy_id, embedded_policy_store),
            embedded_step_continuation,
            local_artifacts.clone(),
            local_receipts.clone(),
        ));
        Self {
            fixture,
            _store_dir: store_dir,
            _artifact_store_dirs: vec![wire_artifact_dir, local_artifact_dir],
            over_the_wire,
            in_process,
            claims: std::sync::Mutex::new(std::collections::BTreeMap::new()),
            children: std::sync::Mutex::new(BTreeMap::new()),
            terminals: std::sync::Mutex::new(BTreeMap::new()),
            uploads: std::sync::Mutex::new(BTreeMap::new()),
            council_leases: std::sync::Mutex::new(BTreeMap::new()),
            intervention_leases: std::sync::Mutex::new(BTreeMap::new()),
            receipt_stores: vec![wire_receipts, local_receipts],
            receipt_artifacts: vec![wire_artifacts, local_artifacts],
            opaque_authorization_targets: std::sync::Mutex::new(BTreeMap::new()),
            artifact_authorizations: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Test-host cache of identities returned by successful claims. Never a store read.
    fn completing(&self, tool: &str, mut arguments: Value) -> Value {
        if tool == "made_record_ceremony_host_handoff" {
            let key = (
                arguments["ceremony_id"].as_str().unwrap().to_owned(),
                arguments["declaration"]["step_id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
            arguments["declaration"]["claim_fence"] = self
                .claims
                .lock()
                .unwrap()
                .get(&key)
                .expect("host saved the original claim")
                .clone();
        }
        let child_id = || {
            self.children
                .lock()
                .unwrap()
                .get(CHILD_PARENT_ID)
                .and_then(|children| children.first())
                .expect("the scripted child preparation returned a child")
                .clone()
        };
        if arguments.get("ceremony_id") == Some(&json!(CHILD_PLACEHOLDER)) {
            arguments["ceremony_id"] = json!(child_id());
        }
        if arguments.get("child_id") == Some(&json!(CHILD_PLACEHOLDER)) {
            arguments["child_id"] = json!(child_id());
        }
        if arguments.get("terminal_event_id") == Some(&json!(TERMINAL_PLACEHOLDER)) {
            arguments["terminal_event_id"] = json!(self
                .terminals
                .lock()
                .unwrap()
                .get(&child_id())
                .expect("the scripted child history returned its terminal"));
        }
        if matches!(
            tool,
            "made_complete_ceremony_step"
                | "made_renew_ceremony_step_lease"
                | "made_complete_execution_receipt"
                | "made_adopt_execution_receipt"
        ) && arguments.get("claim_fence").is_none()
        {
            let key = (
                arguments["ceremony_id"].as_str().unwrap().to_owned(),
                arguments["step_id"].as_str().unwrap().to_owned(),
            );
            arguments["claim_fence"] = self
                .claims
                .lock()
                .unwrap()
                .get(&key)
                .expect("the scripted host captured a claim")
                .clone();
        }
        if tool == "made_start_ceremony_successor" {
            // Every disposition answers for the exact claim the host
            // accepted; the script names the step and the host supplies
            // the fence it captured, exactly as a real caller would.
            let ceremony_id = arguments["ceremony_id"].as_str().unwrap().to_owned();
            if let Some(dispositions) = arguments["dispositions"].as_array_mut() {
                for disposition in dispositions {
                    if disposition.get("claim_fence").is_some() {
                        continue;
                    }
                    let key = (
                        ceremony_id.clone(),
                        disposition["step_id"].as_str().unwrap().to_owned(),
                    );
                    disposition["claim_fence"] = self
                        .claims
                        .lock()
                        .unwrap()
                        .get(&key)
                        .expect("the scripted host captured a claim")
                        .clone();
                }
            }
        }
        if tool == "made_report_ceremony_agent_status"
            && arguments["status"].get("claim_fence").is_none()
        {
            let key = (
                arguments["status"]["ceremony_id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                arguments["status"]["step_id"].as_str().unwrap().to_owned(),
            );
            arguments["status"]["claim_fence"] = self
                .claims
                .lock()
                .unwrap()
                .get(&key)
                .expect("the scripted status host captured a claim")
                .clone();
        }
        arguments
    }

    /// Present the ticket this arm's own pull issued.
    ///
    /// A lease id is minted per engine, so a scripted literal would
    /// hand one engine the other's ticket and compare an
    /// acknowledgement with a refusal.
    fn present_pulled_ticket(
        &self,
        arguments: &Value,
        wire_arguments: &mut Value,
        local_arguments: &mut Value,
    ) {
        if let Some(pulled) = arguments
            .get("delivery_id")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix("$pulled:"))
        {
            let leases = self.intervention_leases.lock().unwrap();
            let [wire, local] = leases.get(pulled).expect("the script pulled this delivery");
            wire_arguments["delivery_id"] = Value::String(wire.0.clone());
            wire_arguments["lease_id"] = Value::String(wire.1.clone());
            local_arguments["delivery_id"] = Value::String(local.0.clone());
            local_arguments["lease_id"] = Value::String(local.1.clone());
        }
    }

    /// Remember the opaque tickets each arm minted for itself.
    ///
    /// A delivery lease and a council lease are the same problem: the
    /// two engines issue their own, and a later call has to present the
    /// one its own arm was given.
    fn remember_issued_tickets(&self, tool: &str, arguments: &Value, wire: &Value, local: &Value) {
        if tool == "made_pull_ceremony_agent_interventions" && !failed(wire) && !failed(local) {
            let ticket = |answer: &Value| {
                let item = &structured(answer)["items"][0];
                (
                    item["delivery_id"].as_str().unwrap_or_default().to_owned(),
                    item["lease_id"].as_str().unwrap_or_default().to_owned(),
                )
            };
            let key = arguments["agent_execution_id"]
                .as_str()
                .expect("a pull names the execution asking")
                .to_owned();
            self.intervention_leases
                .lock()
                .unwrap()
                .insert(key, [ticket(wire), ticket(local)]);
        }
        if tool == "made_await_integrator_attention" && !failed(wire) && !failed(local) {
            let ticket = |answer: &Value| {
                let item = &structured(answer)["items"][0];
                (
                    item["delivery_id"].as_str().unwrap_or_default().to_owned(),
                    item["lease_id"].as_str().unwrap_or_default().to_owned(),
                )
            };
            let (wire_ticket, local_ticket) = (ticket(wire), ticket(local));
            // An empty batch is a legitimate answer and files nothing:
            // the script asks once before there is anything to be
            // handed, and once after.
            if !wire_ticket.0.is_empty() && !local_ticket.0.is_empty() {
                let key = arguments["binding_id"]
                    .as_str()
                    .expect("an await names the binding asking")
                    .to_owned();
                self.intervention_leases
                    .lock()
                    .unwrap()
                    .insert(key, [wire_ticket, local_ticket]);
            }
        }
        if tool == "made_lease_council_events" && !failed(wire) && !failed(local) {
            let key = arguments["consumer"].as_str().unwrap().to_owned();
            let leases = (
                structured(wire)["lease"].clone(),
                structured(local)["lease"].clone(),
            );
            assert!(
                leases.0.is_object() && leases.1.is_object(),
                "script acquires independent free consumers"
            );
            self.council_leases.lock().unwrap().insert(key, leases);
        }
    }

    /// Raw call: omission and malformed-fence tests reach the request gate unchanged.
    async fn call(&self, id: u64, tool: &str, arguments: &Value) -> (Value, Value) {
        let (mut wire_arguments, mut local_arguments) = self.artifact_arguments(arguments);
        if let Some(consumer) = arguments
            .get("lease")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix("$council-lease:"))
        {
            let leases = self.council_leases.lock().unwrap();
            let (wire, local) = leases
                .get(consumer)
                .expect("script acquired this consumer lease");
            wire_arguments["lease"] = wire.clone();
            local_arguments["lease"] = local.clone();
        }
        self.present_pulled_ticket(arguments, &mut wire_arguments, &mut local_arguments);
        self.record_opaque_authorization_targets(id, tool, &wire_arguments, &local_arguments);
        let wire = call_tool(&self.over_the_wire, id, tool, &wire_arguments).await;
        let local = call_tool(&self.in_process, id, tool, &local_arguments).await;
        self.remember_issued_tickets(tool, arguments, &wire, &local);
        if tool == "made_begin_artifact_upload" && !failed(&wire) && !failed(&local) {
            let key = arguments["idempotency_key"]
                .as_str()
                .expect("artifact begin carries an idempotency key")
                .to_owned();
            self.uploads.lock().unwrap().insert(
                key,
                (
                    structured(&wire)["upload_id"]
                        .as_str()
                        .expect("wire begin returns an upload id")
                        .to_owned(),
                    structured(&local)["upload_id"]
                        .as_str()
                        .expect("embedded begin returns an upload id")
                        .to_owned(),
                ),
            );
        }
        if tool == "made_claim_ceremony_step" && !failed(&wire) && !failed(&local) {
            let fence = structured(&wire)["claim_fence"].clone();
            assert_eq!(fence, structured(&local)["claim_fence"]);
            assert_eq!(fence.as_str().unwrap().len(), 64);
            execution_receipts::seed_after_claim(
                &self.receipt_stores,
                &self.receipt_artifacts,
                arguments,
                fence.as_str().unwrap(),
            )
            .await;
            self.claims.lock().unwrap().insert(
                (
                    arguments["ceremony_id"].as_str().unwrap().to_owned(),
                    arguments["step_id"].as_str().unwrap().to_owned(),
                ),
                fence,
            );
        }
        if tool == "made_prepare_ceremony_children" && !failed(&wire) && !failed(&local) {
            let wire_ids = structured(&wire)["child_ids"].clone();
            assert_eq!(wire_ids, structured(&local)["child_ids"]);
            let child_ids = wire_ids
                .as_array()
                .expect("prepared children are an array")
                .iter()
                .map(|id| id.as_str().expect("child id is a string").to_owned())
                .collect();
            self.children.lock().unwrap().insert(
                arguments["ceremony_id"].as_str().unwrap().to_owned(),
                child_ids,
            );
        }
        if tool == "made_read_ceremony_events" && !failed(&wire) && !failed(&local) {
            let terminal = structured(&wire)["records"]
                .as_array()
                .and_then(|records| {
                    records
                        .iter()
                        .find(|record| record["event_type"] == "ceremony_completed")
                })
                .and_then(|record| record["event_id"].as_str());
            if let Some(terminal) = terminal {
                self.terminals.lock().unwrap().insert(
                    arguments["ceremony_id"].as_str().unwrap().to_owned(),
                    terminal.to_owned(),
                );
            }
        }
        (wire, local)
    }

    fn record_opaque_authorization_targets(
        &self,
        id: u64,
        tool: &str,
        wire_arguments: &Value,
        local_arguments: &Value,
    ) {
        if !is_opaque_authorization_target(tool) {
            return;
        }
        self.opaque_authorization_targets.lock().unwrap().insert(
            tool.to_owned(),
            [
                authorization_expectation(id, tool, wire_arguments),
                authorization_expectation(id, tool, local_arguments),
            ],
        );
    }

    fn artifact_arguments(&self, arguments: &Value) -> (Value, Value) {
        let Some(placeholder) = arguments.get("upload_id").and_then(Value::as_str) else {
            return (arguments.clone(), arguments.clone());
        };
        let key = match placeholder {
            ARTIFACT_UPLOAD_PLACEHOLDER => "parity-artifact-upload",
            ABORT_UPLOAD_PLACEHOLDER => "parity-artifact-abort",
            _ => return (arguments.clone(), arguments.clone()),
        };
        let uploads = self.uploads.lock().unwrap();
        let (wire_upload, local_upload) = uploads
            .get(key)
            .unwrap_or_else(|| panic!("the scripted begin did not capture {key}"));
        let mut wire = arguments.clone();
        let mut local = arguments.clone();
        wire["upload_id"] = json!(wire_upload);
        local["upload_id"] = json!(local_upload);
        (wire, local)
    }
}

/// One `tools/call`, returning the JSON-RPC `result` — the success
/// envelope or the error envelope, whichever the server built.
async fn call_tool(server: &MadeMcpServer, id: u64, tool: &str, arguments: &Value) -> Value {
    let traceparent = deterministic_traceparent(id);
    let mut arguments = arguments.clone();
    let object = arguments
        .as_object_mut()
        .expect("parity tool arguments are always objects");
    let metadata = object
        .entry("_meta")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("parity invocation metadata is always an object");
    metadata
        .entry("made_request_id")
        .or_insert_with(|| json!(format!("parity-call-{id}")));
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

fn is_opaque_authorization_target(tool: &str) -> bool {
    matches!(
        tool,
        "made_acknowledge_council_events"
            | "made_release_council_events"
            | "made_put_artifact_chunk"
            | "made_commit_artifact_upload"
            | "made_abort_artifact_upload"
    )
}

fn authorization_expectation(id: u64, tool: &str, arguments: &Value) -> (String, String) {
    let namespace = format!("parity-call-{id}");
    // The request gate drops optional `null` fields before it derives either
    // authorization binding. Council leases expose their unset acknowledgement
    // that way, so reproduce the admitted invocation rather than hashing the
    // caller's pre-gate JSON.
    let accepted_arguments = without_unset_fields(arguments);
    let canonical_arguments = canonical_json(&accepted_arguments);
    let mut digest = Sha256::new();
    digest.update(b"made-mcp-v1\0");
    for field in [
        namespace.as_bytes(),
        b"".as_slice(),
        tool.as_bytes(),
        canonical_arguments.as_bytes(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    (
        ToolTraceContext::authorization_target_digest(tool, &accepted_arguments)
            .as_str()
            .to_owned(),
        format!("made-{:x}", digest.finalize()),
    )
}

fn without_unset_fields(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(_, child)| !child.is_null())
                .map(|(key, child)| (key.clone(), without_unset_fields(child)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_unset_fields).collect()),
        leaf => leaf.clone(),
    }
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(&canonical_value(value)).expect("JSON values always serialize")
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        value => value.clone(),
    }
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

#[tokio::test]
async fn concurrent_claim_options_and_capacity_have_full_mcp_session_parity() {
    let arms = ParityArms::start().await;
    let (remote, embedded) = arms
        .call(
            901,
            "made_start_ceremony",
            &json!({
                "ceremony_id": CONCURRENT_SESSION_ID,
                "definition_yaml": CONCURRENT_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service"
            }),
        )
        .await;
    assert_eq!(remote, embedded);
    assert_eq!(
        structured(&remote)["claimable_step_ids"],
        json!(["a", "b", "c"])
    );
    assert_eq!(structured(&remote)["next_step_id"], "a");

    let (remote, embedded) = arms
        .call(
            902,
            "made_claim_ceremony_step",
            &json!({
                "ceremony_id": CONCURRENT_SESSION_ID,
                "step_id": "a",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parallel-a",
                "lease_ttl_ms": 60_000
            }),
        )
        .await;
    assert_eq!(remote, embedded);
    assert_eq!(
        structured(&remote)["claimable_step_ids"],
        json!(["b", "c"]),
        "one free slot still presents every candidate"
    );

    let (remote, embedded) = arms
        .call(
            903,
            "made_claim_ceremony_step",
            &json!({
                "ceremony_id": CONCURRENT_SESSION_ID,
                "step_id": "c",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parallel-c",
                "lease_ttl_ms": 60_000
            }),
        )
        .await;
    assert_eq!(remote, embedded);
    assert_eq!(structured(&remote)["claimable_step_ids"], json!([]));
    assert_eq!(structured(&remote)["next_step_id"], Value::Null);
}

// ---------------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------------

/// The scripted session, in order. Every shared tool is in here at
/// least once; the coverage assertion below is what keeps it true.
#[allow(clippy::too_many_lines)] // one entry per call; splitting fragments the session
fn session_script() -> Vec<(&'static str, Value)> {
    let mut calls = council_journal::script();
    calls.extend(council_session_script());
    calls.extend(vec![
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
                "context": {
                    "incident_ref": "INC-42", "severity": 2,
                    "next_role": "FACILITATOR"
                },
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
            "made_close_ceremony_intervention",
            json!({
                "ceremony_id": SESSION_ID,
                "intervention_id": "inspect-metrics",
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
        (
            "made_search_ceremony_instances",
            json!({ "id_prefix": "parity-", "limit": 100 }),
        ),
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
        ("made_stream_ceremony", json!({ "ceremony_id": SESSION_ID })),
        (
            "made_stream_ceremony",
            json!({
                "ceremony_id": SESSION_ID,
                "include_agent_activity": true,
                "after_activity_sequence": 0,
                "role_id": "FACILITATOR",
                "wait_timeout_ms": 0,
            }),
        ),
        (
            "made_stream_ceremony",
            json!({
                "ceremony_id": SESSION_ID,
                "after_sequence": 2,
                "max_events": 3,
                "wait_timeout_ms": 0,
            }),
        ),
        // The named global feed is the same durable contract on both
        // surfaces: a read replays, and only the next call's explicit
        // acknowledgement advances it.
        (
            "made_pull_ceremony_events",
            json!({ "consumer": "parity-global-feed", "limit": 2 }),
        ),
        (
            "made_pull_ceremony_events",
            json!({ "consumer": "parity-global-feed", "limit": 2, "acknowledge_through": 1 }),
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
        // Live-agent status is driven by an actual accepted claim, after the
        // committed report so the additional authorization decisions do not
        // churn that report's stable fixture.
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": AGENT_STATUS_SESSION_ID,
                "definition_yaml": PUBLISHED_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service"
            }),
        ),
        (
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": AGENT_STATUS_SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "grpc-fixture-host",
                "idempotency_key": "parity-agent-status-claim",
                "lease_ttl_ms": 60_000
            }),
        ),
        (
            "made_report_ceremony_agent_status",
            json!({
                "status": {
                    "ceremony_id": AGENT_STATUS_SESSION_ID,
                    "agent_execution_id": "parity-agent-execution",
                    "operation_id": ExecutionOperationId::for_step(
                        &CeremonyId::new(AGENT_STATUS_SESSION_ID).unwrap(),
                        &StepId::new("work").unwrap(),
                        StateVisit::FIRST,
                        StateIteration::FIRST,
                        StepIteration::FIRST,
                    ).to_string(),
                    "claim_owner_id": "grpc-fixture-host",
                    "logical_worker_id": "parity-agent-worker",
                    "host_agent_id": "grpc-fixture-host",
                    "host_agent_incarnation": "parity-agent-incarnation-1",
                    "role_id": "FACILITATOR",
                    "step_id": "work",
                    "attempt": 1,
                    "execution_status": "running",
                    "liveness": "fresh",
                    "source": "host_report",
                    "activity": "working",
                    "task_summary": "Running the parity status claim.",
                    "evidence_references": [],
                    "usage_kind": "unavailable",
                    "observed_at": "2026-09-16T09:00:00Z",
                    "report_sequence": 1,
                    "idempotency_key": "parity-agent-status-1"
                }
            }),
        ),
        (
            "made_list_ceremony_agents",
            json!({ "ceremony_id": AGENT_STATUS_SESSION_ID, "limit": 10 }),
        ),
        (
            "made_get_ceremony_agent",
            json!({
                "ceremony_id": AGENT_STATUS_SESSION_ID,
                "agent_execution_id": "parity-agent-execution"
            }),
        ),
        (
            "made_stream_ceremony",
            json!({
                "ceremony_id": AGENT_STATUS_SESSION_ID,
                "include_agent_activity": true,
                "after_activity_sequence": 0,
                "role_id": "FACILITATOR",
                "wait_timeout_ms": 0,
            }),
        ),
        // Budget admission uses a separate published ceremony after the
        // committed report, so exercising the new events cannot perturb its
        // established trace ids or document bytes.
        (
            "made_start_published_ceremony",
            json!({
                "ceremony": "parity_published",
                "version": "1.0",
                "ceremony_id": BUDGET_SESSION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "budget_limits": { "tokens": 100 },
            }),
        ),
        (
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": BUDGET_SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "parity-budget-host",
                "idempotency_key": "parity-budget-work-1",
                "lease_ttl_ms": 60_000,
                "budget_reservation": {
                    "duration": { "quality": "unknown" },
                    "tokens": { "quality": "estimated", "amount": 20 },
                    "cost": { "quality": "unknown" },
                    "tool_calls": { "quality": "unknown" }
                },
            }),
        ),
        (
            "made_get_budget_report",
            json!({ "ceremony_id": BUDGET_SESSION_ID }),
        ),
        (
            "made_list_pending_budget_reservations",
            json!({ "limit": 10 }),
        ),
        // Artifact transfer uses independent stores but fixed metadata,
        // content, digest, artifact ids and timestamps. Only the opaque
        // upload ids are store-minted and normalised above; every byte and
        // every durable record is still compared across both backends.
        (
            "made_begin_artifact_upload",
            json!({
                "requested_artifact_id": "artifact-parity-primary",
                "expected_digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
                "size_bytes": 7,
                "media_type": "text/plain",
                "provenance": {
                    "source_kind": "generated_report",
                    "observed_at": "2026-04-15T12:00:00Z",
                },
                "idempotency_key": "parity-artifact-upload",
            }),
        ),
        (
            "made_put_artifact_chunk",
            json!({
                "upload_id": ARTIFACT_UPLOAD_PLACEHOLDER,
                "offset": 0,
                "bytes_base64": "Zml4dHVyZQ==",
                "chunk_digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
            }),
        ),
        (
            "made_commit_artifact_upload",
            json!({ "upload_id": ARTIFACT_UPLOAD_PLACEHOLDER }),
        ),
        (
            "made_get_artifact",
            json!({ "artifact_id": "artifact-parity-primary" }),
        ),
        ("made_list_artifacts", json!({ "limit": 1 })),
        (
            "made_read_artifact_chunk",
            json!({
                "artifact_id": "artifact-parity-primary",
                "offset": 0,
                "max_bytes": 4,
            }),
        ),
        (
            "made_tombstone_artifact",
            json!({
                "artifact_id": "artifact-parity-primary",
                "actor": "parity-host",
                "policy": "parity-retention",
                "retired_at": "2026-04-15T12:05:00Z",
            }),
        ),
        (
            "made_begin_artifact_upload",
            json!({
                "requested_artifact_id": "artifact-parity-aborted",
                "expected_digest": "sha256:f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d",
                "size_bytes": 7,
                "media_type": "text/plain",
                "provenance": {
                    "source_kind": "generated_report",
                    "observed_at": "2026-04-15T12:00:00Z",
                },
                "idempotency_key": "parity-artifact-abort",
            }),
        ),
        (
            "made_abort_artifact_upload",
            json!({ "upload_id": ABORT_UPLOAD_PLACEHOLDER }),
        ),
        // Lifecycle controls run after the golden report so their trace ids
        // and events cannot perturb the established report fixture.
        (
            "made_pause_ceremony",
            json!({
                "ceremony_id": PUBLISHED_SESSION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "reason": "parity hold",
            }),
        ),
        (
            "made_enforce_ceremony_deadlines",
            json!({ "ceremony_id": PUBLISHED_SESSION_ID }),
        ),
        (
            "made_resume_ceremony",
            json!({
                "ceremony_id": PUBLISHED_SESSION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_cancel_ceremony",
            json!({
                "ceremony_id": PUBLISHED_SESSION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "reason": "parity cancellation",
            }),
        ),
        // Child orchestration runs after the legacy report so exercising the
        // new shared tools cannot perturb its trace ids or sealed hashes.
        (
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": CHILD_CEREMONY }),
        ),
        (
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": CHILD_PARENT_CEREMONY }),
        ),
        (
            "made_start_published_ceremony",
            json!({
                "ceremony": "parity_child_parent",
                "version": "1.0",
                "ceremony_id": CHILD_PARENT_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
                "context": {},
            }),
        ),
        (
            "made_prepare_ceremony_children",
            json!({
                "ceremony_id": CHILD_PARENT_ID,
                "step_id": "delegate",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parity-child-spawn-1",
                "lease_ttl_ms": 60_000,
            }),
        ),
        (
            "made_run_ceremony_step",
            json!({
                "ceremony_id": CHILD_PLACEHOLDER,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "parity-host",
                "idempotency_key": "parity-child-work-1",
                "lease_ttl_ms": 60_000,
            }),
        ),
        (
            "made_apply_ceremony_transition",
            json!({
                "ceremony_id": CHILD_PLACEHOLDER,
                "trigger": "finish",
                "actor_kind": "agent",
            }),
        ),
        (
            "made_read_ceremony_events",
            json!({ "ceremony_id": CHILD_PLACEHOLDER, "from_version": 0, "limit": 200 }),
        ),
        (
            "made_accept_child_completion",
            json!({
                "child_id": CHILD_PLACEHOLDER,
                "terminal_event_id": TERMINAL_PLACEHOLDER,
            }),
        ),
        ("made_recover_ceremony_children", json!({ "limit": 1000 })),
        (
            "made_apply_ceremony_transition",
            json!({
                "ceremony_id": CHILD_PARENT_ID,
                "trigger": "finish",
                "actor_kind": "agent",
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
        ("made_get_authorization_policy", json!({})),
        (
            "made_issue_authorization_grant",
            json!({
                "grant_id": "parity-issued-grant",
                "grantee_id": "grpc-fixture-host",
                "actions": ["get_metrics"],
                "scope": {"kind": "global"},
                "valid_from": "2026-04-15T12:00:00Z",
                "delegation_depth": 0,
                "_meta": {"made_request_id": "parity-admin-issue"},
            }),
        ),
        // Same logical invocation: the transport id changes, while the
        // explicit request namespace and canonical arguments stay exact.
        (
            "made_issue_authorization_grant",
            json!({
                "grant_id": "parity-issued-grant",
                "grantee_id": "grpc-fixture-host",
                "actions": ["get_metrics"],
                "scope": {"kind": "global"},
                "valid_from": "2026-04-15T12:00:00Z",
                "delegation_depth": 0,
                "_meta": {"made_request_id": "parity-admin-issue"},
            }),
        ),
        (
            "made_revoke_authorization_grant",
            json!({
                "grant_id": "parity-issued-grant",
                "reason": "parity retry proof",
                "_meta": {"made_request_id": "parity-admin-revoke"},
            }),
        ),
        (
            "made_revoke_authorization_grant",
            json!({
                "grant_id": "parity-issued-grant",
                "reason": "parity retry proof",
                "_meta": {"made_request_id": "parity-admin-revoke"},
            }),
        ),
        (
            "made_approve_authorization_operation",
            json!({
                "approval_action": "approve_ceremony_guard",
                "execution_action": "mount_definition",
                "scope": {"kind": "global"},
                "target_digest": "b".repeat(64),
                "_meta": {"made_request_id": "parity-admin-approval"},
            }),
        ),
        // Exact retry must return the same durable approval decision.
        (
            "made_approve_authorization_operation",
            json!({
                "approval_action": "approve_ceremony_guard",
                "execution_action": "mount_definition",
                "scope": {"kind": "global"},
                "target_digest": "b".repeat(64),
                "_meta": {"made_request_id": "parity-admin-approval"},
            }),
        ),
        ("made_list_authorization_decisions", json!({"limit": 500})),
    ]);
    calls.extend(execution_receipts::script());
    calls.extend(renewal_script());
    calls.extend(host_handoff::script());
    calls.extend(intervention_delivery::script());
    calls.extend(integrator_loop::script());
    calls.extend(agentic_system::script());
    calls.extend(succession::script());
    calls
}

fn council_session_script() -> Vec<(&'static str, Value)> {
    vec![
        (
            "made_register_agent",
            json!({
                "specialty": "triage",
                "agent": {
                    "agent_id": "agent-triage-0",
                    "specialty": "triage",
                    "kind": "noop",
                    "attributes": { "fixture": "deterministic-noop-agent" },
                },
                "agent_config": { "fixture": "deterministic-noop-agent" },
            }),
        ),
        (
            "made_create_council",
            json!({ "specialty": "triage", "num_agents": 1 }),
        ),
        ("made_list_councils", json!({ "include_agents": true })),
        (
            "made_register_contract",
            json!({
                "contract": {
                    "contract_id": "parity-contract",
                    "format": "json_object",
                    "fields": {
                        "decision": {
                            "required": true,
                            "allowed_string_values": ["accept", "reject"],
                        },
                    },
                    "json_schema": "",
                },
            }),
        ),
        ("made_list_contracts", json!({})),
        (
            "made_deliberate",
            council_task("parity-deliberate", "Assess the parity fixture."),
        ),
        (
            "made_stream_deliberation",
            council_task("parity-stream", "Stream the parity fixture."),
        ),
        (
            "made_get_deliberation_result",
            json!({ "task_id": "parity-deliberate" }),
        ),
        (
            "made_orchestrate",
            json!({
                "task": {
                    "task_id": "parity-orchestrate",
                    "description": "Execute the parity fixture.",
                    "specialty": "triage",
                },
                "execution_options": { "fixture": true },
            }),
        ),
        (
            "made_process_trigger_event",
            json!({
                "event": {
                    "event_id": "parity-trigger",
                    "kind": "parity.requested",
                    "source": "parity-test",
                    "emitted_at": "2026-04-15T12:00:00Z",
                    "requested_specialties": ["triage"],
                    "task_description_template": "Handle the parity trigger.",
                    "payload": { "fixture": true },
                },
            }),
        ),
        (
            "made_run_council_decision",
            json!({
                "contract_id": "parity-contract",
                "specialty": "triage",
                "description": "Choose the parity outcome.",
                "validation_mode": "VALIDATION_MODE_WARN",
            }),
        ),
        (
            "made_delete_contract",
            json!({ "contract_id": "parity-contract" }),
        ),
        ("made_delete_council", json!({ "specialty": "triage" })),
        (
            "made_unregister_agent",
            json!({ "agent_id": "agent-triage-0" }),
        ),
    ]
}

fn renewal_script() -> Vec<(&'static str, Value)> {
    vec![
        (
            "made_renew_ceremony_step_lease",
            json!({"ceremony_id":BUDGET_SESSION_ID,"step_id":"work",
                "lease_owner_id":"parity-budget-host","renewal_id":"budget-heartbeat",
                "lease_ttl_ms":120_000}),
        ),
        (
            "made_renew_ceremony_step_lease",
            json!({"ceremony_id":BUDGET_SESSION_ID,"step_id":"work",
                "lease_owner_id":"parity-budget-host","renewal_id":"budget-heartbeat",
                "lease_ttl_ms":120_000}),
        ),
        (
            "made_get_budget_report",
            json!({"ceremony_id":BUDGET_SESSION_ID}),
        ),
    ]
}

fn council_task(task_id: &str, description: &str) -> Value {
    json!({
        "task": {
            "task_id": task_id,
            "description": description,
            "specialty": "triage",
        },
    })
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
    let called = drive_session_script(arms).await;
    assert_session_is_rich(arms).await;
    assert_transcript_contains_both_steps(arms).await;
    assert_denied_decision_is_visible_through_bounded_pages(arms).await;

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

async fn drive_session_script(arms: &ParityArms) -> BTreeSet<String> {
    let mut called: BTreeSet<String> = BTreeSet::new();
    let mut ceremony_call_id = 0_u64;
    let mut council_call_id = 10_000_u64;
    let mut original_budget = None;

    for (tool, arguments) in session_script() {
        let arguments = arms.completing(tool, arguments);
        let id = if is_council_tool(tool) {
            council_call_id += 1;
            council_call_id
        } else {
            ceremony_call_id += 1;
            ceremony_call_id
        };
        let (over_the_wire, in_process) = arms.call(id, tool, &arguments).await;

        assert!(
            !failed(&over_the_wire),
            "`{tool}` failed on the gRPC backend: {over_the_wire:#}"
        );
        assert!(
            !failed(&in_process),
            "`{tool}` failed on the in-process backend: {in_process:#}"
        );
        if tool == "made_list_authorization_decisions" {
            assert_authorization_decisions(
                &over_the_wire,
                &in_process,
                &arms.opaque_authorization_targets.lock().unwrap(),
                &arms.artifact_authorizations.lock().unwrap(),
            );
        } else if matches!(tool, "made_get_artifact" | "made_list_artifacts") {
            assert_artifact_fact_answer(
                tool,
                &over_the_wire,
                &in_process,
                &arms.opaque_authorization_targets.lock().unwrap(),
                &mut arms.artifact_authorizations.lock().unwrap(),
            );
        } else {
            assert_same_answer(tool, &over_the_wire, &in_process);
        }
        execution_receipts::assert_result(tool, &arguments, structured(&in_process));
        if tool == "made_get_budget_report" && arguments["ceremony_id"] == BUDGET_SESSION_ID {
            let report = structured(&in_process).clone();
            if let Some(before) = &original_budget {
                assert_eq!(
                    &report, before,
                    "heartbeat and replay must not reserve budget twice"
                );
            } else {
                original_budget = Some(report);
            }
        }
        if tool == "made_design_ceremony" {
            let yaml = structured(&in_process)["definition_yaml"]
                .as_str()
                .expect("a designed ceremony carries its YAML");
            assert!(yaml.contains("max_transitions: 12"), "{yaml}");
            assert!(yaml.contains("max_bounces: 3"), "{yaml}");
        }
        if tool == "made_generate_ceremony_report" {
            assert_the_report_is_the_committed_document(structured(&in_process));
        }
        called.insert((*tool).to_owned());
    }
    // The two refusals a handoff owes: every call above has to succeed,
    // and these two have to be refused the same way on both backends.
    succession::assert_both_refuse_a_superseded_origin(arms).await;
    called
}

async fn assert_session_is_rich(arms: &ParityArms) {
    // The session really was as rich as it claims: a collection with
    // nothing in it cannot disagree, and two empty answers would agree
    // about nothing at all.
    let (wire_session, embedded_session) = arms
        .call(
            100,
            "made_get_ceremony_instance",
            &json!({ "ceremony_id": SESSION_ID }),
        )
        .await;
    assert_same_answer(
        "made_get_ceremony_instance",
        &wire_session,
        &embedded_session,
    );
    let session = structured(&embedded_session);
    assert_eq!(session["current_state"], json!("DONE"), "{session:#}");
    assert_eq!(session["steps"].as_array().map(Vec::len), Some(2));
    assert_eq!(session["context"]["last_step"], "work");
    assert_eq!(session["context"]["final_summary"], "the reviewer has it");
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
}

async fn assert_transcript_contains_both_steps(arms: &ParityArms) {
    // Both steps are in the transcript, and one of them is the step
    // the host claimed and completed itself. Until the transcript
    // became a fold of `StepCompleted` (A5) it was a store the two
    // drivers appended to, so the delegated-host protocol left nothing
    // in it and this said one.
    let (transcript_wire, transcript) = arms
        .call(
            101,
            "made_get_ceremony_transcript",
            &json!({ "ceremony_id": SESSION_ID }),
        )
        .await;
    assert_same_answer(
        "made_get_ceremony_transcript",
        &transcript_wire,
        &transcript,
    );
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
}

/// Compare an artifact fact while retaining the authorization that admitted
/// its original commit.
///
/// The stores mint different upload IDs, so the commit authorization has three
/// derived fields that legitimately differ. This checks both complete objects
/// in place, binds each to its arm's exact admitted commit arguments, and saves
/// them for byte-for-byte verification against the public decision ledger.
fn assert_artifact_fact_answer(
    tool: &str,
    wire_answer: &Value,
    embedded_answer: &Value,
    opaque_targets: &BTreeMap<String, [(String, String); 2]>,
    captured: &mut Vec<[Value; 2]>,
) {
    for answer in [wire_answer, embedded_answer] {
        assert_eq!(answer["isError"], json!(false));
        let content = answer["content"]
            .as_array()
            .expect("artifact answers carry MCP content");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], json!("text"));
        let rendered: Value = serde_json::from_str(
            content[0]["text"]
                .as_str()
                .expect("artifact answers carry a JSON text projection"),
        )
        .unwrap();
        assert_eq!(
            rendered, answer["structuredContent"],
            "the artifact text projection must preserve its complete authorization evidence"
        );
    }

    let wire_records = artifact_fact_records(tool, structured(wire_answer));
    let embedded_records = artifact_fact_records(tool, structured(embedded_answer));
    assert_eq!(wire_records.len(), embedded_records.len());
    if tool == "made_list_artifacts" {
        assert_eq!(
            structured(wire_answer)["next_cursor"],
            structured(embedded_answer)["next_cursor"]
        );
    }
    let expectations = opaque_targets
        .get("made_commit_artifact_upload")
        .expect("the artifact fact follows the recorded opaque commit invocation");
    for (wire, embedded) in wire_records.into_iter().zip(embedded_records) {
        assert_eq!(wire["artifact"], embedded["artifact"]);
        assert_eq!(wire["tombstone"], embedded["tombstone"]);
        let wire_authorization = wire
            .get("authorization")
            .expect("new artifact facts retain commit authorization")
            .clone();
        let embedded_authorization = embedded
            .get("authorization")
            .expect("new artifact facts retain commit authorization")
            .clone();
        assert_artifact_authorization_pair(
            &wire_authorization,
            &embedded_authorization,
            expectations,
        );
        captured.push([wire_authorization, embedded_authorization]);
    }
}

fn artifact_fact_records<'a>(tool: &str, answer: &'a Value) -> Vec<&'a Value> {
    match tool {
        "made_get_artifact" => vec![answer],
        "made_list_artifacts" => answer["artifacts"]
            .as_array()
            .expect("artifact listing carries records")
            .iter()
            .collect(),
        _ => panic!("`{tool}` is not an artifact fact reader"),
    }
}

fn assert_artifact_authorization_pair(
    wire: &Value,
    embedded: &Value,
    expectations: &[(String, String); 2],
) {
    for (authorization, (target_digest, request_id)) in
        [(wire, &expectations[0]), (embedded, &expectations[1])]
    {
        let fields = authorization
            .as_object()
            .expect("artifact authorization is an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            fields,
            BTreeSet::from([
                "action",
                "admitted_at",
                "decision_id",
                "policy_version",
                "principal_id",
                "request_id",
                "scope",
                "target_digest",
                "valid_until",
            ])
        );
        assert_eq!(authorization["action"], json!("commit_artifact_upload"));
        assert_eq!(authorization["target_digest"], json!(target_digest));
        assert_eq!(authorization["request_id"], json!(request_id));
        assert_lower_hex(authorization["decision_id"].as_str().unwrap(), 64);
    }
    for field in [
        "action",
        "admitted_at",
        "policy_version",
        "principal_id",
        "scope",
        "valid_until",
    ] {
        assert_eq!(
            wire[field], embedded[field],
            "persisted artifact authorization field `{field}` diverged"
        );
    }
    for field in ["decision_id", "request_id", "target_digest"] {
        assert_ne!(
            wire[field], embedded[field],
            "store-bound artifact authorization field `{field}` unexpectedly matched"
        );
    }
}

fn assert_persisted_artifact_authorizations(
    captured: &[[Value; 2]],
    wire_decisions: &[Value],
    embedded_decisions: &[Value],
) {
    assert!(
        !captured.is_empty(),
        "F4 must observe the commit authorization through artifact readers"
    );
    for arm in 0..2 {
        let decisions = if arm == 0 {
            wire_decisions
        } else {
            embedded_decisions
        };
        let first = &captured[0][arm];
        for pair in captured {
            let authorization = &pair[arm];
            assert_eq!(
                authorization, first,
                "get/list must preserve the original authorization byte-for-byte within one arm"
            );
            let decision = decisions
                .iter()
                .find(|decision| decision["decision_id"] == authorization["decision_id"])
                .expect("persisted artifact authorization links to its public decision");
            assert_eq!(authorization["request_id"], decision["request_id"]);
            assert_eq!(
                authorization["principal_id"],
                decision["principal"]["principal_id"]
            );
            assert_eq!(authorization["action"], decision["action"]);
            assert_eq!(authorization["scope"], decision["scope"]);
            assert_eq!(authorization["target_digest"], decision["target_digest"]);
            assert_eq!(authorization["policy_version"], decision["policy_version"]);
            assert_eq!(authorization["admitted_at"], decision["decided_at"]);
            assert_eq!(authorization["valid_until"], decision["valid_until"]);
        }
    }
}

/// Compare the complete authorization ledger without erasing its bindings.
///
/// Every record must be byte-for-byte equal across the two arms except the
/// five operations whose admitted arguments contain a lease or upload ID
/// minted independently by each store. For those five records, recompute and
/// verify each arm's target digest and request identity from its actual
/// admitted arguments, compare every non-derived field exactly, and require
/// the three derived IDs to differ. No authorization field is normalized or
/// removed from either record.
fn assert_authorization_decisions(
    wire_answer: &Value,
    embedded_answer: &Value,
    opaque_targets: &BTreeMap<String, [(String, String); 2]>,
    artifact_authorizations: &[[Value; 2]],
) {
    for answer in [wire_answer, embedded_answer] {
        let rendered: Value = serde_json::from_str(
            answer["content"][0]["text"]
                .as_str()
                .expect("authorization decisions carry a text projection"),
        )
        .unwrap();
        assert_eq!(
            rendered, answer["structuredContent"],
            "the text projection must preserve every authorization field"
        );
        assert!(
            answer["structuredContent"]["next_after_decision_id"].is_null(),
            "the 500-row F4 page must contain the complete bounded fixture history"
        );
    }
    let mut wire = wire_answer["structuredContent"]["decisions"]
        .as_array()
        .expect("wire decisions are an array")
        .clone();
    let mut embedded = embedded_answer["structuredContent"]["decisions"]
        .as_array()
        .expect("embedded decisions are an array")
        .clone();
    assert_eq!(wire.len(), embedded.len());
    assert_persisted_artifact_authorizations(
        artifact_authorizations,
        wire.as_slice(),
        embedded.as_slice(),
    );

    for (tool, expectations) in opaque_targets {
        let action = tool
            .strip_prefix("made_")
            .expect("opaque target tools use the made_ prefix");
        let wire_decision = take_one_decision(&mut wire, action);
        let embedded_decision = take_one_decision(&mut embedded, action);
        assert_bound_opaque_decision(&wire_decision, action, &expectations[0], &expectations[1]);
        assert_bound_opaque_decision(
            &embedded_decision,
            action,
            &expectations[1],
            &expectations[0],
        );
        for field in [
            "accepted_work_decision_id",
            "action",
            "approval_decision_id",
            "approved_action",
            "decided_at",
            "denial_reason",
            "grant_id",
            "outcome",
            "policy_version",
            "principal",
            "scope",
            "valid_until",
        ] {
            assert_eq!(
                wire_decision[field], embedded_decision[field],
                "opaque-bound decision field `{field}` diverged for `{action}`"
            );
        }
        assert_ne!(wire_decision["request_id"], embedded_decision["request_id"]);
        assert_ne!(
            wire_decision["target_digest"],
            embedded_decision["target_digest"]
        );
        assert_ne!(
            wire_decision["decision_id"],
            embedded_decision["decision_id"]
        );
    }

    if wire != embedded {
        let wire_by_request = decisions_by_request(&wire);
        let embedded_by_request = decisions_by_request(&embedded);
        let differing = wire_by_request
            .keys()
            .chain(embedded_by_request.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|request_id| {
                let wire = wire_by_request.get(request_id);
                let embedded = embedded_by_request.get(request_id);
                (wire != embedded).then(|| {
                    json!({
                        "request_id": request_id,
                        "action": wire.or(embedded).map(|decision| &decision["action"]),
                        "different_fields": differing_decision_fields(wire, embedded),
                    })
                })
            })
            .collect::<Vec<_>>();
        panic!("decisions without store-minted opaque inputs diverged: {differing:#?}");
    }
}

fn differing_decision_fields(
    wire: Option<&&Value>,
    embedded: Option<&&Value>,
) -> Vec<&'static str> {
    let fields = [
        "accepted_work_decision_id",
        "action",
        "approval_decision_id",
        "approved_action",
        "decided_at",
        "decision_id",
        "denial_reason",
        "grant_id",
        "outcome",
        "policy_version",
        "principal",
        "request_id",
        "scope",
        "target_digest",
        "valid_until",
    ];
    fields
        .into_iter()
        .filter(|field| wire.map(|value| &value[*field]) != embedded.map(|value| &value[*field]))
        .collect()
}

fn decisions_by_request(decisions: &[Value]) -> BTreeMap<&str, &Value> {
    decisions
        .iter()
        .map(|decision| {
            (
                decision["request_id"]
                    .as_str()
                    .expect("authorization decisions carry request IDs"),
                decision,
            )
        })
        .collect()
}

fn take_one_decision(decisions: &mut Vec<Value>, action: &str) -> Value {
    let matches = decisions
        .iter()
        .enumerate()
        .filter(|(_, decision)| decision["action"] == json!(action))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "F4 must bind exactly one `{action}` invocation to its opaque input"
    );
    decisions.remove(matches[0])
}

fn assert_bound_opaque_decision(
    decision: &Value,
    action: &str,
    (target_digest, request_id): &(String, String),
    other_expectation: &(String, String),
) {
    let fields = decision
        .as_object()
        .expect("authorization decision is an object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        BTreeSet::from([
            "accepted_work_decision_id",
            "action",
            "approval_decision_id",
            "approved_action",
            "decided_at",
            "decision_id",
            "denial_reason",
            "grant_id",
            "outcome",
            "policy_version",
            "principal",
            "request_id",
            "scope",
            "target_digest",
            "valid_until",
        ])
    );
    assert_eq!(decision["action"], json!(action));
    assert_eq!(
        decision["target_digest"],
        json!(target_digest),
        "`{action}` must bind the authorization decision to its exact invocation target; other arm expected {other_expectation:?}"
    );
    assert_eq!(
        decision["request_id"],
        json!(request_id),
        "`{action}` must bind the authorization decision to its exact invocation identity"
    );
    assert_lower_hex(decision["target_digest"].as_str().unwrap(), 64);
    assert_lower_hex(decision["decision_id"].as_str().unwrap(), 64);
    let request_id = decision["request_id"].as_str().unwrap();
    assert!(request_id.starts_with("made-"));
    assert_lower_hex(&request_id[5..], 64);
}

fn assert_lower_hex(value: &str, length: usize) {
    assert_eq!(value.len(), length);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "`{value}` is not canonical lowercase hex"
    );
}

async fn assert_denied_decision_is_visible_through_bounded_pages(arms: &ParityArms) {
    let revoke = json!({
        "grant_id": "grpc-fixture-business-actions",
        "reason": "prove denied decisions through the public paginated reader",
        "_meta": {"made_request_id": "parity-revoke-business-actions"},
    });
    let (wire, embedded) = arms
        .call(200, "made_revoke_authorization_grant", &revoke)
        .await;
    assert!(!failed(&wire), "gRPC revocation failed: {wire:#}");
    assert!(
        !failed(&embedded),
        "embedded revocation failed: {embedded:#}"
    );
    assert_same_answer("made_revoke_authorization_grant", &wire, &embedded);

    let denied_arguments = json!({
        "_meta": {"made_request_id": "parity-denied-get-metrics"},
    });
    let (wire, embedded) = arms.call(201, "made_get_metrics", &denied_arguments).await;
    assert!(
        failed(&wire),
        "revoked gRPC business grant still admitted get_metrics: {wire:#}"
    );
    assert!(
        failed(&embedded),
        "revoked embedded business grant still admitted get_metrics: {embedded:#}"
    );
    assert_same_answer("made_get_metrics", &wire, &embedded);

    let wire_denial = denied_decision_through_bounded_pages(&arms.over_the_wire, "wire", 202).await;
    let embedded_denial =
        denied_decision_through_bounded_pages(&arms.in_process, "embedded", 402).await;
    assert_eq!(
        wire_denial, embedded_denial,
        "the denied operation precedes the opaque store-minted inputs and must remain exact"
    );
}

async fn denied_decision_through_bounded_pages(
    server: &MadeMcpServer,
    label: &str,
    first_call_id: u64,
) -> Value {
    let mut after: Option<String> = None;
    let mut total_seen = 0_usize;
    let mut denied_actions = BTreeSet::new();
    for page_number in 0_u64..512 {
        let mut arguments = json!({
            "limit": 2,
            "_meta": {
                "made_request_id": format!("parity-decision-page-{label}-{page_number}")
            },
        });
        if let Some(cursor) = &after {
            arguments["after_decision_id"] = json!(cursor);
        }
        let answer = call_tool(
            server,
            first_call_id + page_number,
            "made_list_authorization_decisions",
            &arguments,
        )
        .await;
        assert!(!failed(&answer), "{label} decision page failed: {answer:#}");
        let page = structured(&answer);
        let decisions = page["decisions"]
            .as_array()
            .expect("decision page carries an array");
        total_seen += decisions.len();
        denied_actions.extend(
            decisions
                .iter()
                .filter(|decision| decision["outcome"] == json!("deny"))
                .filter_map(|decision| decision["action"].as_str().map(str::to_owned)),
        );
        assert!(
            decisions.len() <= 2,
            "the public reader exceeded its page bound"
        );
        if let Some(decision) = decisions.iter().find(|decision| {
            decision["action"] == json!("get_metrics") && decision["outcome"] == json!("deny")
        }) {
            return decision.clone();
        }
        let next = page["next_after_decision_id"].as_str().map(str::to_owned);
        if next.is_none() {
            break;
        }
        assert_ne!(next, after, "decision cursor must advance monotonically");
        after = next;
    }
    panic!(
        "bounded {label} authorization-decision pages saw {total_seen} rows and denied actions \
         {denied_actions:?}, but never exposed denied get_metrics"
    );
}

fn is_council_tool(tool: &str) -> bool {
    matches!(
        tool,
        "made_deliberate"
            | "made_stream_deliberation"
            | "made_get_deliberation_result"
            | "made_orchestrate"
            | "made_process_trigger_event"
            | "made_run_council_decision"
            | "made_create_council"
            | "made_list_councils"
            | "made_delete_council"
            | "made_register_agent"
            | "made_unregister_agent"
            | "made_register_contract"
            | "made_list_contracts"
            | "made_delete_contract"
            | "made_read_council_events"
            | "made_get_council_event_cursor"
            | "made_lease_council_events"
            | "made_acknowledge_council_events"
            | "made_release_council_events"
    )
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
        std::fs::write(&path, rendered).expect("the golden document should be writable");
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
        (
            "an estimated budget measurement without an amount",
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "budget_reservation": {
                    "duration": { "quality": "unknown" },
                    "tokens": { "quality": "estimated" },
                    "cost": { "quality": "unknown" },
                    "tool_calls": { "quality": "unknown" }
                }
            }),
        ),
        (
            "a zero budget ceiling beside a positive ceiling",
            "made_start_published_ceremony",
            json!({
                "ceremony_id": "zero-budget",
                "ceremony": "parity_published",
                "version": "1.0",
                "actor_id": "operator",
                "actor_kind": "service",
                "budget_limits": { "tokens": 0, "tool_calls": 10 }
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

    let mut cases = council_error_cases();
    cases.extend(vec![
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
    ]);

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

fn council_error_cases() -> Vec<(&'static str, &'static str, Value, &'static str)> {
    vec![
        (
            "a deliberation for a council that is not there",
            "made_deliberate",
            json!({
                "task": {
                    "task_id": "missing-council",
                    "description": "fail before proposing",
                    "specialty": "unknown",
                },
            }),
            "not_found",
        ),
        (
            "a stream for a council that is not there",
            "made_stream_deliberation",
            json!({
                "task": {
                    "task_id": "missing-stream-council",
                    "description": "fail before the first frame",
                    "specialty": "unknown",
                },
            }),
            "refused",
        ),
        (
            "a council decision naming a contract that is not there",
            "made_run_council_decision",
            json!({
                "contract_id": "no-such-contract",
                "specialty": "unknown",
                "description": "fail before deliberating",
            }),
            "not_found",
        ),
    ]
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
        if tool == "made_run_council_decision" && path.ends_with(".duration_ms") {
            assert!(
                value.as_u64().is_some(),
                "council duration must remain unsigned milliseconds"
            );
        }
        return json!("<normalised>");
    }
    match value {
        Value::String(text) if path == ".content[].text" => {
            let Ok(parsed) = serde_json::from_str::<Value>(text) else {
                return value.clone();
            };
            normalise(&parsed, path, tool)
        }
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

#[test]
fn council_elapsed_time_can_differ_without_hiding_decision_content() {
    let answer = |duration, passed| {
        let body = json!({"duration_ms": duration, "validation": {"passed": passed}});
        json!({"structuredContent": body, "content": [{"text": body.to_string()}]})
    };
    let first = normalise(&answer(0, true), "", "made_run_council_decision");
    assert_eq!(
        first,
        normalise(&answer(7, true), "", "made_run_council_decision")
    );
    assert_ne!(
        first,
        normalise(&answer(7, false), "", "made_run_council_decision")
    );
}

#[test]
#[should_panic(expected = "council duration must remain unsigned milliseconds")]
fn council_elapsed_time_normalisation_refuses_a_broken_wire_type() {
    normalise(
        &json!("7"),
        ".structuredContent.duration_ms",
        "made_run_council_decision",
    );
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
        let arguments = arms.completing(tool, arguments);
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

#[tokio::test]
async fn bounded_progress_collection_matches_over_grpc_and_the_embedded_facade() {
    let arms = ParityArms::start().await;
    let ceremony_id = "progress-parity";
    let (wire, local) = arms
        .call(
            2_100,
            "made_start_ceremony",
            &json!({
                "ceremony_id": ceremony_id,
                "definition_yaml": PUBLISHED_CEREMONY,
                "actor_id": "operator",
                "actor_kind": "service"
            }),
        )
        .await;
    assert_same_answer("made_start_ceremony", &wire, &local);

    let (wire, local) = arms
        .call(
            2_101,
            "made_stream_ceremony",
            &json!({
                "ceremony_id": ceremony_id,
                "after_sequence": 0,
                "max_events": 1,
                "wait_timeout_ms": 0
            }),
        )
        .await;
    assert_same_answer("made_stream_ceremony", &wire, &local);
    let answer = structured(&wire);
    assert_eq!(answer["records"].as_array().unwrap().len(), 1);
    assert_eq!(answer["records"][0]["sequence"], json!(1));
    assert_eq!(answer["resume_after_sequence"], json!(1));
    assert_eq!(answer["head_sequence"], json!(1));
    assert_eq!(answer["end_reason"], json!("event_limit"));
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
        let arguments = arms.completing(tool, arguments);
        arms.call(index as u64 + 1, tool, &arguments).await;
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "the parity session took {elapsed:?}; it runs on every workspace test run"
    );
}

#[tokio::test]
async fn invalid_completion_fences_are_refused_and_an_exact_retry_is_idempotent() {
    let arms = ParityArms::start().await;
    let ceremony_id = "parity-fence-contract";
    let (wire, local) = arms.call(4000, "made_start_ceremony", &json!({"ceremony_id": ceremony_id, "definition_yaml": PUBLISHED_CEREMONY, "actor_id": "operator", "actor_kind": "service"})).await;
    assert_eq!(wire, local);
    assert!(!failed(&wire), "{wire}");
    let (wire, local) = arms.call(4001, "made_claim_ceremony_step", &json!({"ceremony_id": ceremony_id, "step_id": "work", "actor_kind": "agent", "lease_owner_id": "fence-host", "idempotency_key": "fence-claim"})).await;
    assert_eq!(wire, local);
    assert!(!failed(&wire), "{wire}");
    let fence = structured(&wire)["claim_fence"].clone();
    let history_args = json!({"ceremony_id": ceremony_id, "limit": 100});
    let before = arms
        .call(4002, "made_read_ceremony_events", &history_args)
        .await;
    for (index, value) in [None, Some(json!("malformed")), Some(json!("0".repeat(64)))]
        .into_iter()
        .enumerate()
    {
        let mut args = json!({"ceremony_id": ceremony_id, "step_id": "work", "actor_kind": "agent", "status": "completed"});
        if let Some(value) = value {
            args["claim_fence"] = value;
        }
        let (wire, local) = arms
            .call(4010 + index as u64, "made_complete_ceremony_step", &args)
            .await;
        assert_eq!(wire, local);
        assert!(failed(&wire), "{wire}");
        assert_eq!(
            arms.call(4002, "made_read_ceremony_events", &history_args)
                .await,
            before
        );
    }
    let args = json!({"ceremony_id": ceremony_id, "step_id": "work", "actor_kind": "agent", "status": "completed", "claim_fence": fence});
    let (wire, local) = arms.call(4020, "made_complete_ceremony_step", &args).await;
    assert_eq!(wire, local);
    assert!(!failed(&wire), "{wire}");
    let completed = arms
        .call(4022, "made_read_ceremony_events", &history_args)
        .await;
    let (wire, local) = arms.call(4021, "made_complete_ceremony_step", &args).await;
    assert_eq!(wire, local);
    assert!(
        !failed(&wire),
        "an exact completion retry must be idempotent: {wire}"
    );
    assert_eq!(
        arms.call(4022, "made_read_ceremony_events", &history_args)
            .await,
        completed,
        "an exact completion retry must not append another event"
    );
}
