pub(crate) const RUN_CEREMONY_TOOL: &str = "made_run_ceremony";
pub(crate) const START_CEREMONY_TOOL: &str = "made_start_ceremony";
pub(crate) const RUN_CEREMONY_STEP_TOOL: &str = "made_run_ceremony_step";
pub(crate) const PREPARE_CEREMONY_CHILDREN_TOOL: &str = "made_prepare_ceremony_children";
pub(crate) const ACCEPT_CHILD_COMPLETION_TOOL: &str = "made_accept_child_completion";
pub(crate) const RECOVER_CEREMONY_CHILDREN_TOOL: &str = "made_recover_ceremony_children";
pub(crate) const CLAIM_CEREMONY_STEP_TOOL: &str = "made_claim_ceremony_step";
pub(crate) const COMPLETE_CEREMONY_STEP_TOOL: &str = "made_complete_ceremony_step";
pub(crate) const APPROVE_CEREMONY_GUARD_TOOL: &str = "made_approve_ceremony_guard";
pub(crate) const DEFER_CEREMONY_GUARD_TOOL: &str = "made_defer_ceremony_guard";
pub(crate) const APPLY_CEREMONY_TRANSITION_TOOL: &str = "made_apply_ceremony_transition";
pub(crate) const PAUSE_CEREMONY_TOOL: &str = "made_pause_ceremony";
pub(crate) const RESUME_CEREMONY_TOOL: &str = "made_resume_ceremony";
pub(crate) const CANCEL_CEREMONY_TOOL: &str = "made_cancel_ceremony";
pub(crate) const ENFORCE_CEREMONY_DEADLINES_TOOL: &str = "made_enforce_ceremony_deadlines";
pub(crate) const ASSERT_CEREMONY_REASON_TOOL: &str = "made_assert_ceremony_reason";
pub(crate) const GET_CEREMONY_INSTANCE_TOOL: &str = "made_get_ceremony_instance";
pub(crate) const LIST_CEREMONY_INSTANCES_TOOL: &str = "made_list_ceremony_instances";
pub(crate) const REQUEST_CEREMONY_INTERVENTION_TOOL: &str = "made_request_ceremony_intervention";
pub(crate) const RESPOND_TO_CEREMONY_INTERVENTION_TOOL: &str =
    "made_respond_to_ceremony_intervention";
pub(crate) const CLOSE_CEREMONY_INTERVENTION_TOOL: &str = "made_close_ceremony_intervention";
pub(crate) const COLLECT_CEREMONY_EVIDENCE_TOOL: &str = "made_collect_ceremony_evidence";
pub(crate) const DESIGN_CEREMONY_TOOL: &str = "made_design_ceremony";
pub(crate) const READ_CEREMONY_EVENTS_TOOL: &str = "made_read_ceremony_events";
pub(crate) const STREAM_CEREMONY_TOOL: &str = "made_stream_ceremony";
pub(crate) const PULL_CEREMONY_EVENTS_TOOL: &str = "made_pull_ceremony_events";
pub(crate) const GET_CEREMONY_TRANSCRIPT_TOOL: &str = "made_get_ceremony_transcript";
pub(crate) const VERIFY_CEREMONY_JOURNAL_TOOL: &str = "made_verify_ceremony_journal";
pub(crate) const GENERATE_CEREMONY_REPORT_TOOL: &str = "made_generate_ceremony_report";
pub(crate) const BEGIN_ARTIFACT_UPLOAD_TOOL: &str = "made_begin_artifact_upload";
pub(crate) const PUT_ARTIFACT_CHUNK_TOOL: &str = "made_put_artifact_chunk";
pub(crate) const COMMIT_ARTIFACT_UPLOAD_TOOL: &str = "made_commit_artifact_upload";
pub(crate) const ABORT_ARTIFACT_UPLOAD_TOOL: &str = "made_abort_artifact_upload";
pub(crate) const GET_ARTIFACT_TOOL: &str = "made_get_artifact";
pub(crate) const LIST_ARTIFACTS_TOOL: &str = "made_list_artifacts";
pub(crate) const READ_ARTIFACT_CHUNK_TOOL: &str = "made_read_artifact_chunk";
pub(crate) const TOMBSTONE_ARTIFACT_TOOL: &str = "made_tombstone_artifact";
pub(crate) const DISCOVER_CAPABILITIES_TOOL: &str = "made_discover_capabilities";
pub(crate) const GET_HELP_TOOL: &str = "made_get_help";
pub(crate) const VALIDATE_CEREMONY_DRAFT_TOOL: &str = "made_validate_ceremony_draft";
pub(crate) const EXPLAIN_CEREMONY_DRAFT_TOOL: &str = "made_explain_ceremony_draft";
pub(crate) const PUBLISH_CEREMONY_DEFINITION_TOOL: &str = "made_publish_ceremony_definition";
pub(crate) const DIFF_CEREMONY_DEFINITIONS_TOOL: &str = "made_diff_ceremony_definitions";
pub(crate) const BIND_CEREMONY_PARTICIPANTS_TOOL: &str = "made_bind_ceremony_participants";
pub(crate) const START_PUBLISHED_CEREMONY_TOOL: &str = "made_start_published_ceremony";
pub(crate) const GET_STATUS_TOOL: &str = "made_get_status";
pub(crate) const GET_METRICS_TOOL: &str = "made_get_metrics";

pub(super) const GRPC_TOOL_NAMES: [&str; 59] = [
    "made_deliberate",
    "made_stream_deliberation",
    "made_get_deliberation_result",
    "made_orchestrate",
    "made_create_council",
    "made_list_councils",
    "made_delete_council",
    "made_register_agent",
    "made_unregister_agent",
    "made_process_trigger_event",
    "made_run_council_decision",
    "made_register_contract",
    "made_list_contracts",
    "made_delete_contract",
    RUN_CEREMONY_TOOL,
    GET_CEREMONY_INSTANCE_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL,
    START_CEREMONY_TOOL,
    START_PUBLISHED_CEREMONY_TOOL,
    RUN_CEREMONY_STEP_TOOL,
    PREPARE_CEREMONY_CHILDREN_TOOL,
    ACCEPT_CHILD_COMPLETION_TOOL,
    RECOVER_CEREMONY_CHILDREN_TOOL,
    APPLY_CEREMONY_TRANSITION_TOOL,
    PAUSE_CEREMONY_TOOL,
    RESUME_CEREMONY_TOOL,
    CANCEL_CEREMONY_TOOL,
    ENFORCE_CEREMONY_DEADLINES_TOOL,
    APPROVE_CEREMONY_GUARD_TOOL,
    DEFER_CEREMONY_GUARD_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL,
    COLLECT_CEREMONY_EVIDENCE_TOOL,
    ASSERT_CEREMONY_REASON_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL,
    DIFF_CEREMONY_DEFINITIONS_TOOL,
    BIND_CEREMONY_PARTICIPANTS_TOOL,
    CLAIM_CEREMONY_STEP_TOOL,
    COMPLETE_CEREMONY_STEP_TOOL,
    DESIGN_CEREMONY_TOOL,
    READ_CEREMONY_EVENTS_TOOL,
    STREAM_CEREMONY_TOOL,
    PULL_CEREMONY_EVENTS_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL,
    BEGIN_ARTIFACT_UPLOAD_TOOL,
    PUT_ARTIFACT_CHUNK_TOOL,
    COMMIT_ARTIFACT_UPLOAD_TOOL,
    ABORT_ARTIFACT_UPLOAD_TOOL,
    GET_ARTIFACT_TOOL,
    LIST_ARTIFACTS_TOOL,
    READ_ARTIFACT_CHUNK_TOOL,
    TOMBSTONE_ARTIFACT_TOOL,
    GET_STATUS_TOOL,
    GET_METRICS_TOOL,
    VERIFY_CEREMONY_JOURNAL_TOOL,
];

pub(super) const SERVER_TOOL_NAMES: [&str; 2] = [DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL];

pub(crate) fn is_grpc_tool(name: &str) -> bool {
    GRPC_TOOL_NAMES.contains(&name)
}

pub(crate) fn is_server_tool(name: &str) -> bool {
    SERVER_TOOL_NAMES.contains(&name)
}
