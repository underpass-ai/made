use serde_json::{json, Value};

use super::artifact_schemas::{
    artifact_id_schema, artifact_upload_id_schema, begin_artifact_upload_schema,
    list_artifacts_schema, put_artifact_chunk_schema, read_artifact_chunk_schema,
    tombstone_artifact_schema,
};
use super::ceremony_schemas::{
    accept_child_completion_schema, apply_execution_receipt_schema, ceremony_design_schema,
    ceremony_draft_schema, ceremony_guard_approval_schema, ceremony_guard_deferral_schema,
    ceremony_instance_schema, ceremony_lifecycle_control_schema, ceremony_reason_schema,
    ceremony_report_schema, ceremony_search_schema, ceremony_transition_schema,
    claim_ceremony_step_schema, close_ceremony_intervention_schema,
    collect_ceremony_evidence_schema, complete_ceremony_step_schema,
    enforce_ceremony_deadlines_schema, get_ceremony_transcript_schema,
    get_execution_receipt_schema, inspect_execution_recovery_schema,
    prepare_ceremony_children_schema, pull_ceremony_events_schema, read_ceremony_events_schema,
    recover_ceremony_children_schema, request_ceremony_intervention_schema,
    respond_to_ceremony_intervention_schema, run_ceremony_schema, run_ceremony_step_schema,
    start_ceremony_schema, start_published_ceremony_schema, stream_ceremony_schema,
};
use super::general_schemas::{
    agent_summary_schema, empty_object_schema, help_schema, output_contract_schema,
    run_council_decision_schema, task_schema, trigger_event_schema,
};
use super::schema_primitives::{attributes_schema, string_schema, tool_def};
use super::tool_names::{
    is_server_tool, ABORT_ARTIFACT_UPLOAD_TOOL, ACCEPT_CHILD_COMPLETION_TOOL,
    ADOPT_EXECUTION_RECEIPT_TOOL, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL,
    CANCEL_CEREMONY_TOOL, CLAIM_CEREMONY_STEP_TOOL, CLOSE_CEREMONY_INTERVENTION_TOOL,
    COLLECT_CEREMONY_EVIDENCE_TOOL, COMMIT_ARTIFACT_UPLOAD_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL, DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL,
    DISCOVER_CAPABILITIES_TOOL, ENFORCE_CEREMONY_DEADLINES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL, GET_ARTIFACT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL, GET_EXECUTION_RECEIPT_TOOL, GET_HELP_TOOL, GET_METRICS_TOOL,
    GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL, LIST_ARTIFACTS_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, PULL_CEREMONY_EVENTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL,
    READ_ARTIFACT_CHUNK_TOOL, READ_CEREMONY_EVENTS_TOOL, RECOVER_CEREMONY_CHILDREN_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL,
    SEARCH_CEREMONY_INSTANCES_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL,
    STREAM_CEREMONY_TOOL, TOMBSTONE_ARTIFACT_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL,
};

mod authorization_catalog;
mod budget_catalog;
mod ceremony_agent_catalog;
mod ceremony_history_catalog;
mod council_catalog;
mod council_journal_catalog;
mod definition_diff_catalog;
mod host_handoff_catalog;
mod intervention_delivery_catalog;
mod renewal_catalog;

use ceremony_agent_catalog::ceremony_agent_tool_catalog;
use ceremony_history_catalog::verify_ceremony_journal_tool;
use council_catalog::council_tool_catalog;
/// `tools/list` result filtered to capabilities honored by the active
/// backend.
pub(crate) fn tools_list_result(supports: impl Fn(&str) -> bool) -> Value {
    json!({ "tools": available_tool_catalog(supports) })
}

/// Catalog entries executable through this server composition.
///
/// Server-owned introspection tools are available for every backend. All
/// other entries are filtered through the active backend so discovery and
/// `tools/list` can never disagree about the executable surface.
pub(crate) fn available_tool_catalog(supports: impl Fn(&str) -> bool) -> Vec<Value> {
    tool_catalog()
        .into_iter()
        .filter(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| is_server_tool(name) || supports(name))
        })
        .collect()
}

fn tool_catalog() -> Vec<Value> {
    // One list, ordered exactly as the gRPC service orders its RPCs.
    // A test pins that correspondence both ways: a tool with no RPC,
    // or an RPC with no tool, is a surface that exists on one side
    // only, which is how two distributions drift apart.
    let mut tools = grpc_tool_catalog();
    tools.push(tool_def(
        DISCOVER_CAPABILITIES_TOOL,
        "Discover this server's version, active backend, executable tool catalog, capability groups, and artifact generators as machine-readable data.",
        empty_object_schema(),
    ));
    tools.push(tool_def(
        GET_HELP_TOOL,
        "Get audience-specific made guidance. User help explains available workflows and examples; agent help explains preconditions, authority boundaries, delegated-host sequencing, and error handling.",
        help_schema(),
    ));
    tools
}

#[allow(clippy::too_many_lines)] // gRPC tool definitions form one auditable transport contract
pub(super) fn grpc_tool_catalog() -> Vec<Value> {
    let mut tools = council_tool_catalog();
    tools.extend(council_journal_catalog::council_journal_tool_catalog());
    tools.extend([
        tool_def(
            RUN_CEREMONY_TOOL,
            "Execute a declarative ceremony YAML definition and return final state, step trace, and Mermaid sequence diagram.",
            run_ceremony_schema(),
        ),
        tool_def(
            GET_CEREMONY_INSTANCE_TOOL,
            "Inspect a persistent ceremony instance, including step status and blocking guards.",
            ceremony_instance_schema(),
        ),
        tool_def(
            LIST_CEREMONY_INSTANCES_TOOL,
            "Discover ceremony instances available to this backend so a host can resume one after losing its local conversation context.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_def(SEARCH_CEREMONY_INSTANCES_TOOL, "Search one bounded ceremony page.", ceremony_search_schema()),
        tool_def(
            START_CEREMONY_TOOL,
            "Mount a ceremony YAML definition and start a persistent in-process instance without advancing it.",
            start_ceremony_schema(),
        ),
        tool_def(
            START_PUBLISHED_CEREMONY_TOOL,
            "Start a ceremony from a published version, binding the instance to that definition's digest so which one ran can be checked afterwards rather than taken on trust.",
            start_published_ceremony_schema(),
        ),
        tool_def(
            RUN_CEREMONY_STEP_TOOL,
            "Execute one declared step on a started ceremony instance and persist its result.",
            run_ceremony_step_schema(),
        ),
        tool_def(
            PREPARE_CEREMONY_CHILDREN_TOOL,
            "Claim a spawning step, seal its deterministic child plan, open every child exactly once, and complete the parent step only after all openings are verified.",
            prepare_ceremony_children_schema(),
        ),
        tool_def(
            ACCEPT_CHILD_COMPLETION_TOOL,
            "Verify a named CeremonyCompleted record in a child's intact journal, plus its lineage, binding and opening snapshot, before recording it in the parent group.",
            accept_child_completion_schema(),
        ),
        tool_def(
            RECOVER_CEREMONY_CHILDREN_TOOL,
            "Advance durable child-plan and child-completion recovery from its owned event cursor. Notifications may wake this operation but never substitute for cursor evidence.",
            recover_ceremony_children_schema(),
        ),
        tool_def(
            APPLY_CEREMONY_TRANSITION_TOOL,
            "Apply one enabled ceremony transition and return the updated persistent instance.",
            ceremony_transition_schema(),
        ),
        tool_def(
            PAUSE_CEREMONY_TOOL,
            "Pause admission of new ceremony work while already accepted claims and child openings drain.",
            ceremony_lifecycle_control_schema(true),
        ),
        tool_def(
            RESUME_CEREMONY_TOOL,
            "Resume admission of new work without shifting absolute deadlines or live claim fences.",
            ceremony_lifecycle_control_schema(false),
        ),
        host_handoff_catalog::record_tool(),
        host_handoff_catalog::inspect_tool(),
        tool_def(
            CANCEL_CEREMONY_TOOL,
            "Irreversibly end a ceremony without cascading to children or rolling back external work.",
            ceremony_lifecycle_control_schema(true),
        ),
        tool_def(
            ENFORCE_CEREMONY_DEADLINES_TOOL,
            "Evaluate sealed ceremony, state, and step deadlines using the engine clock and persist deterministic timeout decisions.",
            enforce_ceremony_deadlines_schema(),
        ),
        tool_def(
            APPROVE_CEREMONY_GUARD_TOOL,
            "Record an explicit human approval for a currently-blocking human guard. Call only after the human has authorized it.",
            ceremony_guard_approval_schema(),
        ),
        tool_def(
            DEFER_CEREMONY_GUARD_TOOL,
            "Record an explicit human deferral without satisfying the guard or inferring authorization.",
            ceremony_guard_deferral_schema(),
        ),
        tool_def(
            REQUEST_CEREMONY_INTERVENTION_TOOL,
            "Open a participant-requested opinion, investigation, or action on the live ceremony table. This coordinates the request; it does not authorize external mutations.",
            request_ceremony_intervention_schema(),
        ),
        tool_def(
            RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
            "Record one targeted role's response to an open ceremony intervention.",
            respond_to_ceremony_intervention_schema(),
        ),
        tool_def(
            CLOSE_CEREMONY_INTERVENTION_TOOL,
            "Close an open ceremony intervention as its requesting role.",
            close_ceremony_intervention_schema(),
        ),
    ]);
    tools.extend(intervention_delivery_catalog::intervention_delivery_tool_catalog());
    tools.extend([
        tool_def(
            COLLECT_CEREMONY_EVIDENCE_TOOL,
            "Collect a non-empty evidence pack through the configured read-only host source and attach it to an open intervention.",
            collect_ceremony_evidence_schema(),
        ),
        tool_def(
            ASSERT_CEREMONY_REASON_TOOL,
            "Record why one thing this session produced led to another. Only whoever decided something may say what decided them, and only whoever did it may say how; claims about the world are open to any seat, with a stated confidence.",
            ceremony_reason_schema(),
        ),
        tool_def(
            VALIDATE_CEREMONY_DRAFT_TOOL,
            "Analyse a ceremony draft and report every structural defect at once. Read-only: it neither publishes nor executes the draft.",
            ceremony_draft_schema(),
        ),
        tool_def(
            EXPLAIN_CEREMONY_DRAFT_TOOL,
            "Describe what a ceremony draft declares and what would block its publication, in prose meant to be read back and corrected.",
            ceremony_draft_schema(),
        ),
        tool_def(
            PUBLISH_CEREMONY_DEFINITION_TOOL,
            "Fix a validated draft to an immutable version identified by a content digest. Republishing identical content is a no-op; different content under a taken version is refused, never overwritten.",
            ceremony_draft_schema(),
        ),
        definition_diff_catalog::diff_tool(),
        tool_def(
            BIND_CEREMONY_PARTICIPANTS_TOOL,
            "Seat this session's roles: which specialty — and so which council — does each role's work here. A role left unseated is played the way the definition says.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["ceremony_id", "seating", "actor_id", "actor_kind"],
                "properties": {
                    "ceremony_id": string_schema("Session being seated."),
                    "actor_id": string_schema("Who is seating them, in whatever terms you identify callers by. Not a role from the definition: seating the table is done to a session rather than in it, and whoever does it need hold no seat at all."),
                    "actor_kind": {
                        "type": "string",
                        "enum": ["human", "agent", "service", "engine"],
                        "description": "What kind of party that is. Refused when missing or unrecognised."
                    },
                    "seating": {
                        "type": "object",
                        "minProperties": 1,
                        "description": "Role id to specialty. At least one seat; an empty object would change nothing.",
                        "additionalProperties": { "type": "string" }
                    }
                }
            }),
        ),
        tool_def(
            CLAIM_CEREMONY_STEP_TOOL,
            "Acquire a lease for one ceremony step that the MCP host will execute with its own agents and tools. This records the claim but performs no external work. An optional host-owned execution_profile records requested versus actual model, reasoning effort, capabilities, fallback, and host provenance. Retain the returned claim_fence before starting work and send it unchanged when completing.",
            claim_ceremony_step_schema(),
        ),
        tool_def(
            COMPLETE_CEREMONY_STEP_TOOL,
            "Record the observable result and structured output/evidence of one previously claimed host-executed ceremony step. Requires the claim_fence returned by that claim; missing or replaced identities are refused.",
            complete_ceremony_step_schema(),
        ),
        renewal_catalog::renewal_tool(),
    ]);
    tools.extend(ceremony_agent_tool_catalog());
    tools.extend([
        tool_def(
            GET_EXECUTION_RECEIPT_TOOL,
            "Read one immutable terminal execution receipt by its stable operation identity.",
            get_execution_receipt_schema(),
        ),
        tool_def(
            INSPECT_EXECUTION_RECOVERY_TOOL,
            "Page durable operation roots whose receipt has not yet been linked to the ceremony, including persisted intents, any terminal receipt, and the current claim fence.",
            inspect_execution_recovery_schema(),
        ),
        tool_def(
            COMPLETE_EXECUTION_RECEIPT_TOOL,
            "Apply a persisted receipt through the exact claim fence that produced it. A different fence is refused rather than silently adopted.",
            apply_execution_receipt_schema(),
        ),
        tool_def(
            ADOPT_EXECUTION_RECEIPT_TOOL,
            "Explicitly adopt a persisted receipt into a different current claim after reliable operation-id recovery. The producer fence remains unchanged in the receipt.",
            apply_execution_receipt_schema(),
        ),
        tool_def(
            DESIGN_CEREMONY_TOOL,
            "Turn structured intent into a safe linear ceremony YAML draft and analyse it immediately. Read-only: it neither publishes nor starts the ceremony.",
            ceremony_design_schema(),
        ),
        tool_def(
            READ_CEREMONY_EVENTS_TOOL,
            "Read one page of a ceremony's event stream: the sealed records, in order, with their payloads and hash chain. Read-only, and what comes back can be verified as a chain without trusting this server.",
            read_ceremony_events_schema(),
        ),
        tool_def(
            STREAM_CEREMONY_TOOL,
            "Replay and briefly follow one ceremony's sealed event stream, returning a resume cursor and an explicit terminal, event-limit, or wait-elapsed reason.",
            stream_ceremony_schema(),
        ),
        tool_def(
            PULL_CEREMONY_EVENTS_TOOL,
            "Read a named global ceremony-event feed. Reading never commits progress; acknowledge_through explicitly advances it before the page is read.",
            pull_ceremony_events_schema(),
        ),
        tool_def(
            GET_CEREMONY_TRANSCRIPT_TOOL,
            "Read the ordered contributions one ceremony's steps have produced so far. Read-only.",
            get_ceremony_transcript_schema(),
        ),
        tool_def(
            GENERATE_CEREMONY_REPORT_TOOL,
            "Generate a deterministic Markdown report from persisted ceremony state and its audit journal. Read-only: the response contains Markdown and does not persist a file.",
            ceremony_report_schema(),
        ),
        tool_def(
            BEGIN_ARTIFACT_UPLOAD_TOOL,
            "Begin or resume a bounded, digest-declared artifact upload.",
            begin_artifact_upload_schema(),
        ),
        tool_def(
            PUT_ARTIFACT_CHUNK_TOOL,
            "Write one independently digested base64 chunk at the next durable offset.",
            put_artifact_chunk_schema(),
        ),
        tool_def(
            COMMIT_ARTIFACT_UPLOAD_TOOL,
            "Verify the complete size and digest, then publish artifact metadata.",
            artifact_upload_id_schema(),
        ),
        tool_def(
            ABORT_ARTIFACT_UPLOAD_TOOL,
            "Abort incomplete staging without deleting committed content.",
            artifact_upload_id_schema(),
        ),
        tool_def(
            GET_ARTIFACT_TOOL,
            "Read artifact metadata and auditable retention state without blob bytes.",
            artifact_id_schema(),
        ),
        tool_def(
            LIST_ARTIFACTS_TOOL,
            "List a bounded page of artifact metadata using an opaque scoped cursor.",
            list_artifacts_schema(),
        ),
        tool_def(
            READ_ARTIFACT_CHUNK_TOOL,
            "Read a bounded artifact chunk with its digest and continuation offset.",
            read_artifact_chunk_schema(),
        ),
        tool_def(
            TOMBSTONE_ARTIFACT_TOOL,
            "Record host-authorized artifact retention without erasing provenance.",
            tombstone_artifact_schema(),
        ),
        tool_def(
            GET_STATUS_TOOL,
            "Return service health, version, uptime, and optionally statistics.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "include_stats": {
                        "type": "boolean",
                        "description": "When true, include the full Statistics snapshot in the response."
                    }
                }
            }),
        ),
        tool_def(
            GET_METRICS_TOOL,
            "Return the current statistics snapshot.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
    ]);
    tools.push(verify_ceremony_journal_tool());
    budget_catalog::insert_budget_tools(&mut tools);
    authorization_catalog::insert_authorization_tools(&mut tools);
    tools
}
