use crate::protocol::{
    ACCEPT_CHILD_COMPLETION_TOOL, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CANCEL_CEREMONY_TOOL,
    CLAIM_CEREMONY_STEP_TOOL, CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL,
    COMPLETE_CEREMONY_STEP_TOOL, DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL,
    DIFF_CEREMONY_DEFINITIONS_TOOL, DISCOVER_CAPABILITIES_TOOL, ENFORCE_CEREMONY_DEADLINES_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL, GET_HELP_TOOL, GET_METRICS_TOOL, GET_STATUS_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, PULL_CEREMONY_EVENTS_TOOL, READ_CEREMONY_EVENTS_TOOL,
    RECOVER_CEREMONY_CHILDREN_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL,
    RUN_CEREMONY_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL, STREAM_CEREMONY_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
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
            COMPLETE_CEREMONY_STEP_TOOL,
            APPLY_CEREMONY_TRANSITION_TOOL,
            PAUSE_CEREMONY_TOOL,
            RESUME_CEREMONY_TOOL,
            CANCEL_CEREMONY_TOOL,
            ENFORCE_CEREMONY_DEADLINES_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_recovery",
        description: "Rediscover and inspect instances owned by the active backend.",
        tools: &[
            LIST_CEREMONY_INSTANCES_TOOL,
            GET_CEREMONY_INSTANCE_TOOL,
            RECOVER_CEREMONY_CHILDREN_TOOL,
        ],
    },
    CapabilityGroup {
        id: "human_authorization",
        description: "Record explicit guard decisions without inferring human authority.",
        tools: &[APPROVE_CEREMONY_GUARD_TOOL, DEFER_CEREMONY_GUARD_TOOL],
    },
    CapabilityGroup {
        id: "ceremony_participation",
        description:
            "Seat participants, coordinate interventions, attach evidence, and record reasons.",
        tools: &[
            BIND_CEREMONY_PARTICIPANTS_TOOL,
            REQUEST_CEREMONY_INTERVENTION_TOOL,
            RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
            CLOSE_CEREMONY_INTERVENTION_TOOL,
            COLLECT_CEREMONY_EVIDENCE_TOOL,
            ASSERT_CEREMONY_REASON_TOOL,
        ],
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
            "Read the sealed event stream and the transcript a ceremony produced, and verify the chain that seals them.",
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
];
