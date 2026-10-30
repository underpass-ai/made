use crate::mcp_protocol::{
    ABORT_ARTIFACT_UPLOAD_TOOL, ACCEPT_CHILD_COMPLETION_TOOL,
    ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL, ACKNOWLEDGE_PROTOCOL_AGENT_INTERVENTION_TOOL,
    ADOPT_EXECUTION_RECEIPT_TOOL, ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL,
    APPLY_PROTOCOL_TRANSITION_TOOL, APPROVE_PROTOCOL_GUARD_TOOL, ASSERT_PROTOCOL_REASON_TOOL,
    AWAIT_INTEGRATOR_ATTENTION_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL, BIND_PROTOCOL_INTEGRATOR_TOOL,
    BIND_PROTOCOL_PARTICIPANTS_TOOL, CANCEL_PROTOCOL_TOOL, CLAIM_PROTOCOL_STEP_TOOL,
    CLOSE_PROTOCOL_INTERVENTION_TOOL, COLLECT_PROTOCOL_EVIDENCE_TOOL, COMMIT_ARTIFACT_UPLOAD_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL, COMPLETE_PROTOCOL_STEP_TOOL, DEFER_PROTOCOL_GUARD_TOOL,
    DESIGN_AGENTIC_SYSTEM_TOOL, DESIGN_PROTOCOL_TOOL, DIFF_PROTOCOL_DEFINITIONS_TOOL,
    DISCOVER_CAPABILITIES_TOOL, ENFORCE_PROTOCOL_DEADLINES_TOOL, EXPLAIN_PROTOCOL_DRAFT_TOOL,
    GENERATE_PROTOCOL_REPORT_TOOL, GET_AGENTIC_SYSTEM_EXECUTION_TOOL, GET_AGENTIC_SYSTEM_TOOL,
    GET_ARTIFACT_TOOL, GET_BUDGET_REPORT_TOOL, GET_EXECUTION_RECEIPT_TOOL, GET_HELP_TOOL,
    GET_METRICS_TOOL, GET_PROTOCOL_DEFINITION_TOOL, GET_PROTOCOL_INSTANCE_TOOL,
    GET_PROTOCOL_INTEGRATOR_BINDING_TOOL, GET_PROTOCOL_INTERVENTION_TOOL,
    GET_PROTOCOL_TRANSCRIPT_TOOL, GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL,
    INSTANTIATE_AGENTIC_SYSTEM_TOOL, LIST_AGENTIC_SYSTEMS_TOOL, LIST_ARTIFACTS_TOOL,
    LIST_ATTENTION_DELIVERIES_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
    LIST_PROTOCOL_DEFINITIONS_TOOL, LIST_PROTOCOL_INSTANCES_TOOL, LIST_PROTOCOL_INTERVENTIONS_TOOL,
    PAUSE_PROTOCOL_TOOL, PREPARE_PROTOCOL_CHILDREN_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL,
    PUBLISH_PROTOCOL_DEFINITION_TOOL, PULL_PROTOCOL_AGENT_INTERVENTIONS_TOOL,
    PULL_PROTOCOL_EVENTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL, READ_ARTIFACT_CHUNK_TOOL,
    READ_PROTOCOL_EVENTS_TOOL, RECOVER_PROTOCOL_CHILDREN_TOOL, RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
    REQUEST_PROTOCOL_INTERVENTION_TOOL, RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
    RESUME_PROTOCOL_TOOL, RUN_PROTOCOL_STEP_TOOL, RUN_PROTOCOL_TOOL,
    SEARCH_PROTOCOL_INSTANCES_TOOL, START_PROTOCOL_TOOL, START_PUBLISHED_PROTOCOL_TOOL,
    STREAM_PROTOCOL_TOOL, TOMBSTONE_ARTIFACT_TOOL, VALIDATE_AGENTIC_SYSTEM_TOOL,
    VALIDATE_PROTOCOL_DRAFT_TOOL, VERIFY_PROTOCOL_JOURNAL_TOOL,
};

pub(crate) struct CapabilityGroup {
    pub(crate) id: &'static str,
    pub(super) description: &'static str,
    pub(crate) tools: &'static [&'static str],
}

pub(crate) const CAPABILITY_GROUPS: &[CapabilityGroup] = &[
    CapabilityGroup {
        id: "self_description",
        description: "Inspect the active server and obtain audience-specific help.",
        tools: &[DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL],
    },
    CapabilityGroup {
        id: "council_deliberation",
        description: "Run, inspect, trigger, and optionally execute council deliberations.",
        tools: &[
            "made_deliberate",
            "made_stream_deliberation",
            "made_get_deliberation_result",
            "made_orchestrate",
            "made_process_trigger_event",
            "made_run_council_decision",
        ],
    },
    CapabilityGroup {
        id: "council_configuration",
        description: "Manage councils, agents, and output contracts.",
        tools: &[
            "made_create_council",
            "made_list_councils",
            "made_delete_council",
            "made_register_agent",
            "made_unregister_agent",
            "made_register_contract",
            "made_list_contracts",
            "made_delete_contract",
        ],
    },
    CapabilityGroup {
        id: "council_journal",
        description: "Read the independent council journal and manage fenced durable consumer cursors.",
        tools: &["made_read_council_events","made_get_council_event_cursor","made_lease_council_events","made_acknowledge_council_events","made_release_council_events"],
    },
    CapabilityGroup {
        id: "protocol_design",
        description: "Design, validate, explain, compare, publish and read back protocol definitions.",
        tools: &[
            DESIGN_PROTOCOL_TOOL,
            VALIDATE_PROTOCOL_DRAFT_TOOL,
            EXPLAIN_PROTOCOL_DRAFT_TOOL,
            PUBLISH_PROTOCOL_DEFINITION_TOOL,
            DIFF_PROTOCOL_DEFINITIONS_TOOL,
            LIST_PROTOCOL_DEFINITIONS_TOOL,
            GET_PROTOCOL_DEFINITION_TOOL,
        ],
    },
    // After protocol design and before protocol execution, because
    // that is the order the work happens in: a system composes
    // published protocols, and there is nothing to compose until
    // they exist.
    CapabilityGroup {
        id: "agentic_system_design",
        description: "Design, validate, seal, run and draw a system of roles, participants and composed protocols.",
        tools: &[
            DESIGN_AGENTIC_SYSTEM_TOOL,
            GET_AGENTIC_SYSTEM_TOOL,
            LIST_AGENTIC_SYSTEMS_TOOL,
            VALIDATE_AGENTIC_SYSTEM_TOOL,
            PUBLISH_AGENTIC_SYSTEM_TOOL,
            INSTANTIATE_AGENTIC_SYSTEM_TOOL,
            ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL,
            GET_AGENTIC_SYSTEM_EXECUTION_TOOL,
            RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
        ],
    },
    CapabilityGroup {
        id: "protocol_execution",
        description: "Run a protocol in one shot or drive a persistent instance step by step.",
        tools: &[
            RUN_PROTOCOL_TOOL,
            START_PROTOCOL_TOOL,
            START_PUBLISHED_PROTOCOL_TOOL,
            RUN_PROTOCOL_STEP_TOOL,
            PREPARE_PROTOCOL_CHILDREN_TOOL,
            ACCEPT_CHILD_COMPLETION_TOOL,
            CLAIM_PROTOCOL_STEP_TOOL,
            crate::mcp_protocol::RENEW_PROTOCOL_STEP_LEASE_TOOL,
            COMPLETE_PROTOCOL_STEP_TOOL,
            APPLY_PROTOCOL_TRANSITION_TOOL,
            PAUSE_PROTOCOL_TOOL,
            RESUME_PROTOCOL_TOOL,
            crate::mcp_protocol::RECORD_PROTOCOL_HOST_HANDOFF_TOOL,
            crate::mcp_protocol::INSPECT_PROTOCOL_RESUME_TOOL,
            CANCEL_PROTOCOL_TOOL,
            ENFORCE_PROTOCOL_DEADLINES_TOOL,
        ],
    },
    CapabilityGroup {
        id: "protocol_recovery",
        description: "Rediscover and inspect instances owned by the active backend, recover interrupted executions, and hand a paused protocol whose definition turned out to be wrong to an auditable successor.",
        tools: &[
            SEARCH_PROTOCOL_INSTANCES_TOOL,
            LIST_PROTOCOL_INSTANCES_TOOL,
            GET_PROTOCOL_INSTANCE_TOOL,
            RECOVER_PROTOCOL_CHILDREN_TOOL,
            GET_EXECUTION_RECEIPT_TOOL,
            INSPECT_EXECUTION_RECOVERY_TOOL,
            COMPLETE_EXECUTION_RECEIPT_TOOL,
            ADOPT_EXECUTION_RECEIPT_TOOL,
            crate::mcp_protocol::PLAN_PROTOCOL_SUCCESSOR_TOOL,
            crate::mcp_protocol::START_PROTOCOL_SUCCESSOR_TOOL,
        ],
    },
    CapabilityGroup {
        id: "protocol_agent_visibility",
        description: "Read/report bounded live agent status; protocol history can follow its filtered, resumable activity beside sealed engine progress.",
        tools: &["made_list_protocol_agents", "made_get_protocol_agent", "made_report_protocol_agent_status"],
    },
    CapabilityGroup {
        id: "human_authorization",
        description: "Record explicit guard decisions without inferring human authority.",
        tools: &[APPROVE_PROTOCOL_GUARD_TOOL, DEFER_PROTOCOL_GUARD_TOOL],
    },
    CapabilityGroup {
        id: "protocol_participation",
        description:
            "Seat participants, put interventions to the agents that can answer them, acknowledge and answer what arrives, attach evidence, and record reasons.",
        tools: &[
            BIND_PROTOCOL_PARTICIPANTS_TOOL,
            REQUEST_PROTOCOL_INTERVENTION_TOOL,
            RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
            CLOSE_PROTOCOL_INTERVENTION_TOOL,
            PULL_PROTOCOL_AGENT_INTERVENTIONS_TOOL,
            ACKNOWLEDGE_PROTOCOL_AGENT_INTERVENTION_TOOL,
            GET_PROTOCOL_INTERVENTION_TOOL,
            LIST_PROTOCOL_INTERVENTIONS_TOOL,
            COLLECT_PROTOCOL_EVIDENCE_TOOL,
            ASSERT_PROTOCOL_REASON_TOOL,
        ],
    },
    // After participation and before authorization administration,
    // because the loop is how a host takes part in a whole scope
    // rather than in one exchange: the same delivery ledger, one level
    // up. It is the last group about doing the work; what follows is
    // about who may.
    CapabilityGroup {
        id: "integrator_loop",
        description:
            "Bind one host to a protocol or a system run, be handed what it is owed, and say what was done about it. Delivery to a host is transport, never evidence that anybody acted.",
        tools: &[
            BIND_PROTOCOL_INTEGRATOR_TOOL,
            GET_PROTOCOL_INTEGRATOR_BINDING_TOOL,
            AWAIT_INTEGRATOR_ATTENTION_TOOL,
            ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
            LIST_ATTENTION_DELIVERIES_TOOL,
        ],
    },
    CapabilityGroup {
        id: "authorization_administration",
        description: "Inspect policy and decision evidence, issue scoped grants, and revoke authority.",
        tools: &["made_get_authorization_policy", "made_issue_authorization_grant", "made_revoke_authorization_grant", "made_approve_authorization_operation", "made_list_authorization_decisions"],
    },
    CapabilityGroup {
        id: "service_observability",
        description: "Inspect service health and statistics.",
        tools: &[GET_STATUS_TOOL, GET_METRICS_TOOL],
    },
    // Its own group rather than part of `protocol_reporting`: reading
    // the sealed records and the transcript answers "what happened",
    // and what a caller does with that is its own business. Reporting
    // answers "give me the document", which is one particular thing to
    // do with it, and a host may want to offer either without the
    // other.
    CapabilityGroup {
        id: "protocol_history",
        description:
            "Read sealed engine progress, follow bounded live agent activity, inspect the transcript, and verify the chain that seals engine records.",
        tools: &[
            READ_PROTOCOL_EVENTS_TOOL,
            STREAM_PROTOCOL_TOOL,
            PULL_PROTOCOL_EVENTS_TOOL,
            VERIFY_PROTOCOL_JOURNAL_TOOL,
            GET_PROTOCOL_TRANSCRIPT_TOOL,
        ],
    },
    CapabilityGroup {
        id: "protocol_reporting",
        description: "Project persisted protocol state and journals into Markdown.",
        tools: &[GENERATE_PROTOCOL_REPORT_TOOL],
    },
    CapabilityGroup {
        id: "protocol_budgets",
        description: "Read durable tree budget balances and pending reservation recovery pages.",
        tools: &[
            GET_BUDGET_REPORT_TOOL,
            LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
        ],
    },
    CapabilityGroup {
        id: "artifact_transfer",
        description: "Upload, verify, page, read, and retire bounded durable artifacts.",
        tools: &[
            BEGIN_ARTIFACT_UPLOAD_TOOL,
            PUT_ARTIFACT_CHUNK_TOOL,
            COMMIT_ARTIFACT_UPLOAD_TOOL,
            ABORT_ARTIFACT_UPLOAD_TOOL,
            GET_ARTIFACT_TOOL,
            LIST_ARTIFACTS_TOOL,
            READ_ARTIFACT_CHUNK_TOOL,
            TOMBSTONE_ARTIFACT_TOOL,
        ],
    },
];
