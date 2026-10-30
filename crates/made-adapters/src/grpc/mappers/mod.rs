//! Proto ↔ domain conversions.
//!
//! Every conversion is fallible in one direction (proto → domain can
//! fail validation) and infallible the other way (domain values are
//! already validated). Errors surface as [`DomainError`] so the RPC
//! handler can funnel them through the common
//! [`crate::grpc::domain_error_to_status`] mapping.
//!
//! [`DomainError`]: made_core::error::DomainError

mod actor_kind;
mod agent;
pub(crate) mod agentic_system;
mod artifact;
mod attributes;
mod authorization;
mod budget;
mod context;
mod council;
pub(crate) mod council_journal;
mod deliberation;
mod event;
mod execution_receipt;
pub(crate) mod host_handoff;
mod integrator_loop_requests;
mod integrator_loop_state;
mod intervention_delivery;
mod intervention_request_options;
mod output_contract;
mod phase;
mod proposal;
mod protocol_agent_status;
mod protocol_authoring;
mod protocol_authorization;
mod protocol_catalogue;
mod protocol_delegation;
mod protocol_design;
mod protocol_design_stage;
mod protocol_history;
mod protocol_human_verbs;
mod protocol_instance;
mod protocol_instance_children;
mod protocol_instance_interventions;
mod protocol_instance_step;
mod protocol_lifecycle;
pub(crate) mod protocol_preflight;
mod protocol_progress;
pub(crate) mod protocol_succession;
mod run_council_decision;
mod run_protocol;
mod task;
mod timestamp;
mod validation;

// Helpers wired by the service module.
pub use artifact::{
    artifact_chunk_to_proto, artifact_page_limit_from_proto, artifact_record_to_proto,
    artifact_ref_to_proto, artifact_tombstone_to_proto, artifact_upload_status_to_proto,
    begin_artifact_upload_from_proto, put_artifact_chunk_from_proto,
    read_artifact_chunk_from_proto, tombstone_artifact_from_proto,
};
pub(super) use attributes::attributes_from_struct;
pub use authorization::{
    authorization_action_from_proto, authorization_decision_to_proto,
    authorization_grant_from_proto, authorization_policy_to_proto, authorization_scope_from_proto,
};
pub use budget::{
    budget_balance_to_proto, budget_limits_from_proto, budget_reservation_estimate_from_proto,
    budget_reservation_to_proto,
};
pub(super) use council::council_summary_from;
pub(super) use deliberation::{deliberate_response_from, orchestrate_response_from};
pub(super) use event::trigger_event_from_proto;
pub use execution_receipt::{
    adopt_execution_receipt_input_from_proto, complete_execution_receipt_input_from_proto,
    execution_receipt_to_proto, execution_recovery_cursor_from_proto,
    execution_recovery_limit_from_proto, execution_recovery_page_to_proto,
};
pub use integrator_loop_requests::{
    acknowledge_integrator_attention_input_from_proto, await_integrator_attention_input_from_proto,
    bind_protocol_integrator_input_from_proto, integrator_scope_from_proto,
    list_attention_deliveries_input_from_proto,
};
pub use integrator_loop_state::{
    attention_batch_to_proto, attention_delivery_page_to_proto, bind_outcome_to_proto,
    integrator_acknowledged_to_proto, integrator_binding_state,
};
pub use intervention_delivery::{
    acknowledge_protocol_agent_intervention_input_from_proto,
    get_protocol_intervention_input_from_proto, intervention_delivery_state,
    intervention_page_to_proto, list_protocol_interventions_input_from_proto,
    pull_protocol_agent_interventions_input_from_proto, pulled_interventions_to_proto,
};
pub(super) use output_contract::{output_contract_from_proto, output_contract_to_proto};
pub(super) use phase::proto_phase_from_domain;
pub use protocol_agent_status::{
    page_to_proto as protocol_agent_status_page_to_proto,
    query_from_proto as protocol_agent_status_query_from_proto,
    status_from_proto as protocol_agent_status_from_proto,
    status_to_proto as protocol_agent_status_to_proto,
};
pub use protocol_authoring::{
    diff_protocol_definitions_response_from, explain_protocol_draft_response_from,
    protocol_definition_source_from_proto, publish_protocol_definition_response_from,
    validate_protocol_draft_response_from,
};
pub use protocol_catalogue::{
    get_protocol_definition_response_from, list_protocol_definitions_response_from,
    published_protocol_definition_identity_from_proto,
    published_protocol_definition_query_from_proto,
};
pub use protocol_delegation::{
    claim_protocol_step_input_from_proto, complete_protocol_step_input_from_proto,
};
pub use protocol_design::{design_protocol_response_from, protocol_design_document_from_proto};
pub use protocol_history::{
    generate_protocol_report_response_from, get_protocol_transcript_response_from,
    pull_protocol_events_response_from, read_protocol_events_response_from,
    verify_protocol_journal_response_from,
};
pub use protocol_human_verbs::{
    approve_protocol_guard_input_from_proto, assert_protocol_reason_input_from_proto,
    bind_protocol_participants_input_from_proto, close_protocol_intervention_input_from_proto,
    collect_protocol_evidence_input_from_proto, defer_protocol_guard_input_from_proto,
    request_protocol_intervention_input_from_proto,
    respond_to_protocol_intervention_input_from_proto,
};
pub use protocol_instance::{
    protocol_instance_state_from, unrehydratable_protocol_instance_state_from,
};
pub use protocol_instance_children::child_completion_state_from;
pub use protocol_lifecycle::{
    apply_protocol_transition_input_from_proto, cancel_protocol_input_from_proto,
    enforce_protocol_deadlines_input_from_proto, pause_protocol_input_from_proto,
    resume_protocol_input_from_proto, run_protocol_step_input_from_proto,
    start_protocol_from_proto, start_published_protocol_input_from_proto, StartProtocolFromYaml,
};
pub use protocol_progress::stream_protocol_response_from;
pub(super) use run_council_decision::{
    run_council_decision_input_from_proto, run_council_decision_response_from,
};
pub(super) use run_protocol::{run_protocol_input_from_proto, run_protocol_response_from};
pub(super) use task::task_from_proto;
pub(super) use timestamp::{offset_to_timestamp, timestamp_to_offset};
