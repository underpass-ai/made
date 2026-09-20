use crate::protocol::{
    ABORT_ARTIFACT_UPLOAD_TOOL, ACCEPT_CHILD_COMPLETION_TOOL,
    ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL, ADOPT_EXECUTION_RECEIPT_TOOL,
    ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, APPLY_CEREMONY_TRANSITION_TOOL,
    APPROVE_CEREMONY_GUARD_TOOL, ASSERT_CEREMONY_REASON_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL,
    BIND_CEREMONY_PARTICIPANTS_TOOL, CANCEL_CEREMONY_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMMIT_ARTIFACT_UPLOAD_TOOL,
    COMPLETE_CEREMONY_STEP_TOOL, COMPLETE_EXECUTION_RECEIPT_TOOL, DEFER_CEREMONY_GUARD_TOOL,
    DESIGN_AGENTIC_SYSTEM_TOOL, DESIGN_CEREMONY_TOOL, DIFF_CEREMONY_DEFINITIONS_TOOL,
    DISCOVER_CAPABILITIES_TOOL, ENFORCE_CEREMONY_DEADLINES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL, GET_AGENTIC_SYSTEM_EXECUTION_TOOL, GET_AGENTIC_SYSTEM_TOOL,
    GET_ARTIFACT_TOOL, GET_BUDGET_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_INTERVENTION_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL, GET_EXECUTION_RECEIPT_TOOL,
    GET_HELP_TOOL, GET_METRICS_TOOL, GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL,
    INSTANTIATE_AGENTIC_SYSTEM_TOOL, LIST_AGENTIC_SYSTEMS_TOOL, LIST_ARTIFACTS_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, LIST_CEREMONY_INTERVENTIONS_TOOL,
    LIST_PENDING_BUDGET_RESERVATIONS_TOOL, PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL,
    PUBLISH_AGENTIC_SYSTEM_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL, PULL_CEREMONY_EVENTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL,
    READ_ARTIFACT_CHUNK_TOOL, READ_CEREMONY_EVENTS_TOOL, RECOVER_CEREMONY_CHILDREN_TOOL,
    RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL,
    RUN_CEREMONY_TOOL, SEARCH_CEREMONY_INSTANCES_TOOL, START_CEREMONY_TOOL,
    START_PUBLISHED_CEREMONY_TOOL, STREAM_CEREMONY_TOOL, TOMBSTONE_ARTIFACT_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
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
        id: "ceremony_design",
        description: "Design, validate, explain, compare, and publish ceremony definitions.",
        tools: &[
            DESIGN_CEREMONY_TOOL,
            VALIDATE_CEREMONY_DRAFT_TOOL,
            EXPLAIN_CEREMONY_DRAFT_TOOL,
            PUBLISH_CEREMONY_DEFINITION_TOOL,
            DIFF_CEREMONY_DEFINITIONS_TOOL,
        ],
    },
    // After ceremony design and before ceremony execution, because
    // that is the order the work happens in: a system composes
    // published ceremonies, and there is nothing to compose until
    // they exist.
    CapabilityGroup {
        id: "agentic_system_design",
        description: "Design, validate, seal, run and draw a system of roles, participants and composed ceremonies.",
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
        id: "ceremony_execution",
        description: "Run a ceremony in one shot or drive a persistent instance step by step.",
        tools: &[
            RUN_CEREMONY_TOOL,
            START_CEREMONY_TOOL,
            START_PUBLISHED_CEREMONY_TOOL,
            RUN_CEREMONY_STEP_TOOL,
            PREPARE_CEREMONY_CHILDREN_TOOL,
            ACCEPT_CHILD_COMPLETION_TOOL,
            CLAIM_CEREMONY_STEP_TOOL,
            crate::protocol::RENEW_CEREMONY_STEP_LEASE_TOOL,
            COMPLETE_CEREMONY_STEP_TOOL,
            APPLY_CEREMONY_TRANSITION_TOOL,
            PAUSE_CEREMONY_TOOL,
            RESUME_CEREMONY_TOOL,
            crate::protocol::RECORD_CEREMONY_HOST_HANDOFF_TOOL,
            crate::protocol::INSPECT_CEREMONY_RESUME_TOOL,
            CANCEL_CEREMONY_TOOL,
            ENFORCE_CEREMONY_DEADLINES_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_recovery",
        description: "Rediscover and inspect instances owned by the active backend.",
        tools: &[
            SEARCH_CEREMONY_INSTANCES_TOOL,
            LIST_CEREMONY_INSTANCES_TOOL,
            GET_CEREMONY_INSTANCE_TOOL,
            RECOVER_CEREMONY_CHILDREN_TOOL,
            GET_EXECUTION_RECEIPT_TOOL,
            INSPECT_EXECUTION_RECOVERY_TOOL,
            COMPLETE_EXECUTION_RECEIPT_TOOL,
            ADOPT_EXECUTION_RECEIPT_TOOL,
            crate::protocol::PLAN_CEREMONY_SUCCESSOR_TOOL,
            crate::protocol::START_CEREMONY_SUCCESSOR_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_agent_visibility",
        description: "Read/report bounded live agent status; ceremony history can follow its filtered, resumable activity beside sealed engine progress.",
        tools: &["made_list_ceremony_agents", "made_get_ceremony_agent", "made_report_ceremony_agent_status"],
    },
    CapabilityGroup {
        id: "human_authorization",
        description: "Record explicit guard decisions without inferring human authority.",
        tools: &[APPROVE_CEREMONY_GUARD_TOOL, DEFER_CEREMONY_GUARD_TOOL],
    },
    CapabilityGroup {
        id: "ceremony_participation",
        description:
            "Seat participants, put interventions to the agents that can answer them, acknowledge and answer what arrives, attach evidence, and record reasons.",
        tools: &[
            BIND_CEREMONY_PARTICIPANTS_TOOL,
            REQUEST_CEREMONY_INTERVENTION_TOOL,
            RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
            CLOSE_CEREMONY_INTERVENTION_TOOL,
            PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL,
            ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL,
            GET_CEREMONY_INTERVENTION_TOOL,
            LIST_CEREMONY_INTERVENTIONS_TOOL,
            COLLECT_CEREMONY_EVIDENCE_TOOL,
            ASSERT_CEREMONY_REASON_TOOL,
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
    // Its own group rather than part of `ceremony_reporting`: reading
    // the sealed records and the transcript answers "what happened",
    // and what a caller does with that is its own business. Reporting
    // answers "give me the document", which is one particular thing to
    // do with it, and a host may want to offer either without the
    // other.
    CapabilityGroup {
        id: "ceremony_history",
        description:
            "Read sealed engine progress, follow bounded live agent activity, inspect the transcript, and verify the chain that seals engine records.",
        tools: &[
            READ_CEREMONY_EVENTS_TOOL,
            STREAM_CEREMONY_TOOL,
            PULL_CEREMONY_EVENTS_TOOL,
            VERIFY_CEREMONY_JOURNAL_TOOL,
            GET_CEREMONY_TRANSCRIPT_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_reporting",
        description: "Project persisted ceremony state and journals into Markdown.",
        tools: &[GENERATE_CEREMONY_REPORT_TOOL],
    },
    CapabilityGroup {
        id: "ceremony_budgets",
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
