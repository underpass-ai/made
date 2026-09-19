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
mod artifact;
mod attributes;
mod ceremony_authoring;
mod ceremony_delegation;
mod ceremony_design;
mod ceremony_design_stage;
mod ceremony_history;
mod ceremony_human_verbs;
mod ceremony_instance;
mod ceremony_instance_children;
mod ceremony_lifecycle;
mod ceremony_progress;
mod context;
mod council;
pub(crate) mod council_journal;
mod deliberation;
mod event;
mod execution_receipt;
mod output_contract;
mod phase;
mod proposal;
mod run_ceremony;
mod run_council_decision;
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
pub use ceremony_authoring::{
    ceremony_definition_source_from_proto, diff_ceremony_definitions_response_from,
    explain_ceremony_draft_response_from, publish_ceremony_definition_response_from,
    validate_ceremony_draft_response_from,
};
pub use ceremony_delegation::{
    claim_ceremony_step_input_from_proto, complete_ceremony_step_input_from_proto,
};
pub use ceremony_design::{ceremony_design_document_from_proto, design_ceremony_response_from};
pub use ceremony_history::{
    generate_ceremony_report_response_from, get_ceremony_transcript_response_from,
    pull_ceremony_events_response_from, read_ceremony_events_response_from,
    verify_ceremony_journal_response_from,
};
pub use ceremony_human_verbs::{
    approve_ceremony_guard_input_from_proto, assert_ceremony_reason_input_from_proto,
    bind_ceremony_participants_input_from_proto, close_ceremony_intervention_input_from_proto,
    collect_ceremony_evidence_input_from_proto, defer_ceremony_guard_input_from_proto,
    request_ceremony_intervention_input_from_proto,
    respond_to_ceremony_intervention_input_from_proto,
};
pub use ceremony_instance::{
    ceremony_instance_state_from, unrehydratable_ceremony_instance_state_from,
};
pub use ceremony_instance_children::child_completion_state_from;
pub use ceremony_lifecycle::{
    apply_ceremony_transition_input_from_proto, cancel_ceremony_input_from_proto,
    enforce_ceremony_deadlines_input_from_proto, pause_ceremony_input_from_proto,
    resume_ceremony_input_from_proto, run_ceremony_step_input_from_proto,
    start_ceremony_from_proto, start_published_ceremony_input_from_proto, StartCeremonyFromYaml,
};
pub use ceremony_progress::stream_ceremony_response_from;
pub(super) use council::council_summary_from;
pub(super) use deliberation::{deliberate_response_from, orchestrate_response_from};
pub(super) use event::trigger_event_from_proto;
pub use execution_receipt::{
    complete_execution_receipt_input_from_proto, execution_receipt_to_proto,
    execution_recovery_cursor_from_proto, execution_recovery_limit_from_proto,
    execution_recovery_page_to_proto,
};
pub(super) use output_contract::{output_contract_from_proto, output_contract_to_proto};
pub(super) use phase::proto_phase_from_domain;
pub(super) use run_ceremony::{run_ceremony_input_from_proto, run_ceremony_response_from};
pub(super) use run_council_decision::{
    run_council_decision_input_from_proto, run_council_decision_response_from,
};
pub(super) use task::task_from_proto;
pub(super) use timestamp::offset_to_timestamp;
