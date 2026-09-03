use serde_json::{json, Value};

use super::ceremony_schemas::{
    ceremony_definition_ref_schema, ceremony_design_schema, ceremony_draft_schema,
    ceremony_guard_approval_schema, ceremony_guard_deferral_schema, ceremony_instance_schema,
    ceremony_reason_schema, ceremony_report_schema, ceremony_transition_schema,
    claim_ceremony_step_schema, close_ceremony_intervention_schema,
    collect_ceremony_evidence_schema, complete_ceremony_step_schema,
    request_ceremony_intervention_schema, respond_to_ceremony_intervention_schema,
    run_ceremony_schema, run_ceremony_step_schema, start_ceremony_schema,
    start_published_ceremony_schema,
};
use super::general_schemas::{
    agent_summary_schema, empty_object_schema, help_schema, output_contract_schema,
    run_council_decision_schema, task_schema, trigger_event_schema,
};
use super::schema_primitives::{string_schema, tool_def};
use super::tool_names::{
    is_server_tool, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL, DIFF_CEREMONY_DEFINITIONS_TOOL,
    DISCOVER_CAPABILITIES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, GET_HELP_TOOL, LIST_CEREMONY_INSTANCES_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL,
    START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL,
};

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
        DESIGN_CEREMONY_TOOL,
        "Turn structured intent into a safe linear ceremony YAML draft and analyse it immediately. Read-only: it neither publishes nor starts the ceremony.",
        ceremony_design_schema(),
    ));
    tools.push(tool_def(
        CLAIM_CEREMONY_STEP_TOOL,
        "Acquire a lease for one ceremony step that the MCP host will execute with its own agents and tools. This records the claim but performs no external work.",
        claim_ceremony_step_schema(),
    ));
    tools.push(tool_def(
        COMPLETE_CEREMONY_STEP_TOOL,
        "Record the observable result and structured output/evidence of one previously claimed host-executed ceremony step.",
        complete_ceremony_step_schema(),
    ));
    tools.push(tool_def(
        GENERATE_CEREMONY_REPORT_TOOL,
        "Generate a deterministic Markdown report from persisted ceremony state and its audit journal. Read-only: the response contains Markdown and does not persist a file.",
        ceremony_report_schema(),
    ));
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
    vec![
        tool_def(
            "made_deliberate",
            "Run a deliberation on the council for the task's specialty. Returns ranked proposals once the council finishes.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": { "task": task_schema() }
            }),
        ),
        tool_def(
            "made_stream_deliberation",
            "Run a deliberation and return every phase-transition / result frame buffered into a single response array (no live streaming over MCP stdio).",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": { "task": task_schema() }
            }),
        ),
        tool_def(
            "made_get_deliberation_result",
            "Fetch a previously-executed deliberation by task id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task_id"],
                "properties": { "task_id": string_schema("Stable task id used at deliberation time.") }
            }),
        ),
        tool_def(
            "made_orchestrate",
            "Deliberate AND execute the winning proposal through the wired executor port. Returns the winner plus an execution id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": {
                    "task": task_schema(),
                    "execution_options": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Opaque executor options. Forwarded verbatim to the configured ExecutorPort."
                    }
                }
            }),
        ),
        tool_def(
            "made_create_council",
            "Create or replace the council for a specialty. `agent_config` is opaque and passed to the agent factory.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty", "num_agents"],
                "properties": {
                    "specialty": string_schema("Free-form specialty label, e.g. \"triage\"."),
                    "num_agents": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Number of agents to seat on the council."
                    },
                    "agent_config": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Opaque config forwarded to the agent factory."
                    }
                }
            }),
        ),
        tool_def(
            "made_list_councils",
            "List the councils registered on the made.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "include_agents": {
                        "type": "boolean",
                        "description": "When true, return each council's agent roster."
                    }
                }
            }),
        ),
        tool_def(
            "made_delete_council",
            "Delete the council registered for a specialty. Idempotent: `deleted=false` means the council did not exist.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty"],
                "properties": { "specialty": string_schema("Specialty whose council to delete.") }
            }),
        ),
        tool_def(
            "made_register_agent",
            "Register an agent on a council. `agent.kind` must be one supported by the wired AgentFactoryPort (e.g. noop, anthropic, openai, vllm).",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty", "agent"],
                "properties": {
                    "specialty": string_schema("Specialty the agent belongs to."),
                    "agent": agent_summary_schema(),
                    "agent_config": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Opaque per-agent factory config."
                    }
                }
            }),
        ),
        tool_def(
            "made_unregister_agent",
            "Unregister a previously-registered agent by id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["agent_id"],
                "properties": { "agent_id": string_schema("Agent id returned by made_register_agent.") }
            }),
        ),
        tool_def(
            "made_process_trigger_event",
            "Submit a domain event that should fan out to one or more deliberations. Returns a TriggerAck reporting the dispatched task ids.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["event"],
                "properties": { "event": trigger_event_schema() }
            }),
        ),
        tool_def(
            "made_run_council_decision",
            "Run a council deliberation against a registered output contract and return the validated winner plus candidate breakdown.",
            run_council_decision_schema(),
        ),
        tool_def(
            "made_register_contract",
            "Register an `OutputContract` in the in-memory contract registry.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["contract"],
                "properties": { "contract": output_contract_schema() }
            }),
        ),
        tool_def(
            "made_list_contracts",
            "List every contract registered in the made.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_def(
            "made_delete_contract",
            "Delete a registered contract by id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["contract_id"],
                "properties": { "contract_id": string_schema("Contract id previously returned by register_contract.") }
            }),
        ),
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
            APPLY_CEREMONY_TRANSITION_TOOL,
            "Apply one enabled ceremony transition and return the updated persistent instance.",
            ceremony_transition_schema(),
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
        tool_def(
            DIFF_CEREMONY_DEFINITIONS_TOOL,
            "Compare two ceremony definitions and say what changed — and, for each change, whether a session already running the earlier one could go on.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["before", "after"],
                "properties": {
                    "before": ceremony_definition_ref_schema("The earlier definition."),
                    "after": ceremony_definition_ref_schema("The later definition.")
                }
            }),
        ),
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
                        "description": "Role id to specialty. At least one seat; an empty object would change nothing.",
                        "additionalProperties": { "type": "string" }
                    }
                }
            }),
        ),
        tool_def(
            "made_get_status",
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
            "made_get_metrics",
            "Return the current statistics snapshot.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
    ]
}
