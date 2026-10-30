pub(crate) const RUN_PROTOCOL_TOOL: &str = "made_run_protocol";
pub(crate) const START_PROTOCOL_TOOL: &str = "made_start_protocol";
pub(crate) const RUN_PROTOCOL_STEP_TOOL: &str = "made_run_protocol_step";
pub(crate) const PREPARE_PROTOCOL_CHILDREN_TOOL: &str = "made_prepare_protocol_children";
pub(crate) const ACCEPT_CHILD_COMPLETION_TOOL: &str = "made_accept_child_completion";
pub(crate) const RECOVER_PROTOCOL_CHILDREN_TOOL: &str = "made_recover_protocol_children";
pub(crate) const CLAIM_PROTOCOL_STEP_TOOL: &str = "made_claim_protocol_step";
pub(crate) const RENEW_PROTOCOL_STEP_LEASE_TOOL: &str = "made_renew_protocol_step_lease";
pub(crate) const COMPLETE_PROTOCOL_STEP_TOOL: &str = "made_complete_protocol_step";
pub(crate) const GET_EXECUTION_RECEIPT_TOOL: &str = "made_get_execution_receipt";
pub(crate) const INSPECT_EXECUTION_RECOVERY_TOOL: &str = "made_inspect_execution_recovery";
pub(crate) const COMPLETE_EXECUTION_RECEIPT_TOOL: &str = "made_complete_execution_receipt";
pub(crate) const ADOPT_EXECUTION_RECEIPT_TOOL: &str = "made_adopt_execution_receipt";
pub(crate) const APPROVE_PROTOCOL_GUARD_TOOL: &str = "made_approve_protocol_guard";
pub(crate) const DEFER_PROTOCOL_GUARD_TOOL: &str = "made_defer_protocol_guard";
pub(crate) const APPLY_PROTOCOL_TRANSITION_TOOL: &str = "made_apply_protocol_transition";
pub(crate) const PAUSE_PROTOCOL_TOOL: &str = "made_pause_protocol";
pub(crate) const RESUME_PROTOCOL_TOOL: &str = "made_resume_protocol";
pub(crate) const RECORD_PROTOCOL_HOST_HANDOFF_TOOL: &str = "made_record_protocol_host_handoff";
pub(crate) const INSPECT_PROTOCOL_RESUME_TOOL: &str = "made_inspect_protocol_resume";
pub(crate) const PLAN_PROTOCOL_SUCCESSOR_TOOL: &str = "made_plan_protocol_successor";
pub(crate) const START_PROTOCOL_SUCCESSOR_TOOL: &str = "made_start_protocol_successor";
pub(crate) const CANCEL_PROTOCOL_TOOL: &str = "made_cancel_protocol";
pub(crate) const ENFORCE_PROTOCOL_DEADLINES_TOOL: &str = "made_enforce_protocol_deadlines";
pub(crate) const ASSERT_PROTOCOL_REASON_TOOL: &str = "made_assert_protocol_reason";
pub(crate) const GET_PROTOCOL_INSTANCE_TOOL: &str = "made_get_protocol_instance";
pub(crate) const LIST_PROTOCOL_INSTANCES_TOOL: &str = "made_list_protocol_instances";
pub(crate) const SEARCH_PROTOCOL_INSTANCES_TOOL: &str = "made_search_protocol_instances";
pub(crate) const REQUEST_PROTOCOL_INTERVENTION_TOOL: &str = "made_request_protocol_intervention";
pub(crate) const RESPOND_TO_PROTOCOL_INTERVENTION_TOOL: &str =
    "made_respond_to_protocol_intervention";
pub(crate) const CLOSE_PROTOCOL_INTERVENTION_TOOL: &str = "made_close_protocol_intervention";
pub(crate) const PULL_PROTOCOL_AGENT_INTERVENTIONS_TOOL: &str =
    "made_pull_protocol_agent_interventions";
pub(crate) const ACKNOWLEDGE_PROTOCOL_AGENT_INTERVENTION_TOOL: &str =
    "made_acknowledge_protocol_agent_intervention";
pub(crate) const GET_PROTOCOL_INTERVENTION_TOOL: &str = "made_get_protocol_intervention";
pub(crate) const LIST_PROTOCOL_INTERVENTIONS_TOOL: &str = "made_list_protocol_interventions";
pub(crate) const BIND_PROTOCOL_INTEGRATOR_TOOL: &str = "made_bind_protocol_integrator";
pub(crate) const GET_PROTOCOL_INTEGRATOR_BINDING_TOOL: &str =
    "made_get_protocol_integrator_binding";
pub(crate) const AWAIT_INTEGRATOR_ATTENTION_TOOL: &str = "made_await_integrator_attention";
pub(crate) const ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL: &str =
    "made_acknowledge_integrator_attention";
pub(crate) const LIST_ATTENTION_DELIVERIES_TOOL: &str = "made_list_attention_deliveries";
pub(crate) const COLLECT_PROTOCOL_EVIDENCE_TOOL: &str = "made_collect_protocol_evidence";
pub(crate) const DESIGN_PROTOCOL_TOOL: &str = "made_design_protocol";
pub(crate) const READ_PROTOCOL_EVENTS_TOOL: &str = "made_read_protocol_events";
pub(crate) const STREAM_PROTOCOL_TOOL: &str = "made_stream_protocol";
pub(crate) const PULL_PROTOCOL_EVENTS_TOOL: &str = "made_pull_protocol_events";
pub(crate) const GET_PROTOCOL_TRANSCRIPT_TOOL: &str = "made_get_protocol_transcript";
pub(crate) const VERIFY_PROTOCOL_JOURNAL_TOOL: &str = "made_verify_protocol_journal";
pub(crate) const GENERATE_PROTOCOL_REPORT_TOOL: &str = "made_generate_protocol_report";
pub(crate) const GET_BUDGET_REPORT_TOOL: &str = "made_get_budget_report";
pub(crate) const LIST_PENDING_BUDGET_RESERVATIONS_TOOL: &str =
    "made_list_pending_budget_reservations";
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
pub(crate) const VALIDATE_PROTOCOL_DRAFT_TOOL: &str = "made_validate_protocol_draft";
pub(crate) const EXPLAIN_PROTOCOL_DRAFT_TOOL: &str = "made_explain_protocol_draft";
pub(crate) const PUBLISH_PROTOCOL_DEFINITION_TOOL: &str = "made_publish_protocol_definition";
pub(crate) const DIFF_PROTOCOL_DEFINITIONS_TOOL: &str = "made_diff_protocol_definitions";
pub(crate) const LIST_PROTOCOL_DEFINITIONS_TOOL: &str = "made_list_protocol_definitions";
pub(crate) const GET_PROTOCOL_DEFINITION_TOOL: &str = "made_get_protocol_definition";
pub(crate) const DESIGN_AGENTIC_SYSTEM_TOOL: &str = "made_design_agentic_system";
pub(crate) const GET_AGENTIC_SYSTEM_TOOL: &str = "made_get_agentic_system";
pub(crate) const LIST_AGENTIC_SYSTEMS_TOOL: &str = "made_list_agentic_systems";
pub(crate) const VALIDATE_AGENTIC_SYSTEM_TOOL: &str = "made_validate_agentic_system";
pub(crate) const PUBLISH_AGENTIC_SYSTEM_TOOL: &str = "made_publish_agentic_system";
pub(crate) const INSTANTIATE_AGENTIC_SYSTEM_TOOL: &str = "made_instantiate_agentic_system";
pub(crate) const ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL: &str =
    "made_advance_agentic_system_execution";
pub(crate) const GET_AGENTIC_SYSTEM_EXECUTION_TOOL: &str = "made_get_agentic_system_execution";
pub(crate) const RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL: &str = "made_render_agentic_system_diagram";
pub(crate) const BIND_PROTOCOL_PARTICIPANTS_TOOL: &str = "made_bind_protocol_participants";
pub(crate) const START_PUBLISHED_PROTOCOL_TOOL: &str = "made_start_published_protocol";
pub(crate) const GET_STATUS_TOOL: &str = "made_get_status";
pub(crate) const GET_METRICS_TOOL: &str = "made_get_metrics";
pub(crate) const LIST_PROTOCOL_AGENTS_TOOL: &str = "made_list_protocol_agents";
pub(crate) const GET_PROTOCOL_AGENT_TOOL: &str = "made_get_protocol_agent";
pub(crate) const REPORT_PROTOCOL_AGENT_STATUS_TOOL: &str = "made_report_protocol_agent_status";

pub(crate) const GRPC_TOOL_NAMES: [&str; 104] = [
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
    "made_read_council_events",
    "made_get_council_event_cursor",
    "made_lease_council_events",
    "made_acknowledge_council_events",
    "made_release_council_events",
    RUN_PROTOCOL_TOOL,
    GET_PROTOCOL_INSTANCE_TOOL,
    LIST_PROTOCOL_INSTANCES_TOOL,
    SEARCH_PROTOCOL_INSTANCES_TOOL,
    START_PROTOCOL_TOOL,
    START_PUBLISHED_PROTOCOL_TOOL,
    RUN_PROTOCOL_STEP_TOOL,
    PREPARE_PROTOCOL_CHILDREN_TOOL,
    ACCEPT_CHILD_COMPLETION_TOOL,
    RECOVER_PROTOCOL_CHILDREN_TOOL,
    APPLY_PROTOCOL_TRANSITION_TOOL,
    PAUSE_PROTOCOL_TOOL,
    RESUME_PROTOCOL_TOOL,
    RECORD_PROTOCOL_HOST_HANDOFF_TOOL,
    INSPECT_PROTOCOL_RESUME_TOOL,
    PLAN_PROTOCOL_SUCCESSOR_TOOL,
    START_PROTOCOL_SUCCESSOR_TOOL,
    CANCEL_PROTOCOL_TOOL,
    ENFORCE_PROTOCOL_DEADLINES_TOOL,
    APPROVE_PROTOCOL_GUARD_TOOL,
    DEFER_PROTOCOL_GUARD_TOOL,
    REQUEST_PROTOCOL_INTERVENTION_TOOL,
    RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
    CLOSE_PROTOCOL_INTERVENTION_TOOL,
    PULL_PROTOCOL_AGENT_INTERVENTIONS_TOOL,
    ACKNOWLEDGE_PROTOCOL_AGENT_INTERVENTION_TOOL,
    GET_PROTOCOL_INTERVENTION_TOOL,
    LIST_PROTOCOL_INTERVENTIONS_TOOL,
    BIND_PROTOCOL_INTEGRATOR_TOOL,
    GET_PROTOCOL_INTEGRATOR_BINDING_TOOL,
    AWAIT_INTEGRATOR_ATTENTION_TOOL,
    ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
    LIST_ATTENTION_DELIVERIES_TOOL,
    COLLECT_PROTOCOL_EVIDENCE_TOOL,
    ASSERT_PROTOCOL_REASON_TOOL,
    VALIDATE_PROTOCOL_DRAFT_TOOL,
    EXPLAIN_PROTOCOL_DRAFT_TOOL,
    PUBLISH_PROTOCOL_DEFINITION_TOOL,
    DIFF_PROTOCOL_DEFINITIONS_TOOL,
    LIST_PROTOCOL_DEFINITIONS_TOOL,
    GET_PROTOCOL_DEFINITION_TOOL,
    DESIGN_AGENTIC_SYSTEM_TOOL,
    GET_AGENTIC_SYSTEM_TOOL,
    LIST_AGENTIC_SYSTEMS_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL,
    PUBLISH_AGENTIC_SYSTEM_TOOL,
    INSTANTIATE_AGENTIC_SYSTEM_TOOL,
    ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL,
    GET_AGENTIC_SYSTEM_EXECUTION_TOOL,
    RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
    BIND_PROTOCOL_PARTICIPANTS_TOOL,
    CLAIM_PROTOCOL_STEP_TOOL,
    COMPLETE_PROTOCOL_STEP_TOOL,
    RENEW_PROTOCOL_STEP_LEASE_TOOL,
    LIST_PROTOCOL_AGENTS_TOOL,
    GET_PROTOCOL_AGENT_TOOL,
    REPORT_PROTOCOL_AGENT_STATUS_TOOL,
    GET_EXECUTION_RECEIPT_TOOL,
    INSPECT_EXECUTION_RECOVERY_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL,
    ADOPT_EXECUTION_RECEIPT_TOOL,
    DESIGN_PROTOCOL_TOOL,
    READ_PROTOCOL_EVENTS_TOOL,
    STREAM_PROTOCOL_TOOL,
    PULL_PROTOCOL_EVENTS_TOOL,
    GET_PROTOCOL_TRANSCRIPT_TOOL,
    GENERATE_PROTOCOL_REPORT_TOOL,
    GET_BUDGET_REPORT_TOOL,
    LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
    BEGIN_ARTIFACT_UPLOAD_TOOL,
    PUT_ARTIFACT_CHUNK_TOOL,
    COMMIT_ARTIFACT_UPLOAD_TOOL,
    ABORT_ARTIFACT_UPLOAD_TOOL,
    GET_ARTIFACT_TOOL,
    LIST_ARTIFACTS_TOOL,
    READ_ARTIFACT_CHUNK_TOOL,
    TOMBSTONE_ARTIFACT_TOOL,
    "made_get_authorization_policy",
    "made_approve_authorization_operation",
    "made_issue_authorization_grant",
    "made_revoke_authorization_grant",
    "made_list_authorization_decisions",
    GET_STATUS_TOOL,
    GET_METRICS_TOOL,
    VERIFY_PROTOCOL_JOURNAL_TOOL,
];

pub(super) const SERVER_TOOL_NAMES: [&str; 2] = [DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL];

pub(crate) fn is_grpc_tool(name: &str) -> bool {
    GRPC_TOOL_NAMES.contains(&name)
}

pub(crate) fn is_server_tool(name: &str) -> bool {
    SERVER_TOOL_NAMES.contains(&name)
}
