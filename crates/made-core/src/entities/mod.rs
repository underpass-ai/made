//! Domain entities and aggregates.
//!
//! Entities have identity that persists across state changes. Aggregate
//! roots own invariants spanning multiple objects; state transitions
//! happen through their methods, not by mutating fields directly.

mod agent_execution_status;
mod agent_liveness;
mod agent_status_source;
mod agentic_system;
mod agentic_system_analysis;
mod agentic_system_execution;
mod agentic_system_publication_outcome;
mod agentic_system_validation_report;
mod audit_chain;
mod audit_fact;
mod audit_record;
mod authorization_policy;
mod authorization_policy_event;
mod authorization_policy_helpers;
mod authorized_audit_fact;
mod budget_ledger;
mod budget_ledger_event;
mod ceremony_agent_activity;
mod ceremony_agent_activity_kind;
mod ceremony_agent_status;
mod ceremony_command;
pub mod ceremony_commands;
mod ceremony_definition;
mod ceremony_definition_analysis;
mod ceremony_definition_draft;
mod ceremony_event;
mod ceremony_event_reader;
pub mod ceremony_events;
mod ceremony_evidence_pack;
mod ceremony_instance;
mod ceremony_intervention;
mod context_item;
mod context_reference;
mod context_summary;
mod council;
mod deliberation;
mod deliberation_phase;
mod external_context;
mod external_context_validation;
mod metric_family;
mod metric_sample;
mod metrics_snapshot;
mod proposal;
mod publication_outcome;
mod published_agentic_system;
mod published_ceremony_definition;
mod ranked_outcome;
mod statistics;
mod task;
mod task_constraints;
mod task_metadata;
mod validation;
mod validator_report;

#[cfg(test)]
mod authorization_policy_tests;
#[cfg(test)]
mod budget_ledger_tests;

pub use agent_execution_status::AgentExecutionStatus;
pub use agent_liveness::AgentLiveness;
pub use agent_status_source::AgentStatusSource;
pub use agentic_system::{AgenticSystem, AgenticSystemParts};
pub use agentic_system_analysis::AgenticSystemAnalysis;
pub use agentic_system_execution::AgenticSystemExecution;
pub use agentic_system_publication_outcome::AgenticSystemPublicationOutcome;
pub use agentic_system_validation_report::AgenticSystemValidationReport;
pub use audit_chain::AuditChain;
pub use audit_fact::AuditFact;
pub use audit_record::{AuditRecord, AUDIT_RECORD_SCHEMA_VERSION};
pub use authorization_policy::AuthorizationPolicy;
pub use authorization_policy_event::AuthorizationPolicyEvent;
pub use authorized_audit_fact::AuthorizedAuditFact;
pub use budget_ledger::BudgetLedger;
pub use budget_ledger_event::BudgetLedgerEvent;
pub use ceremony_agent_activity::CeremonyAgentActivity;
pub use ceremony_agent_activity_kind::CeremonyAgentActivityKind;
pub use ceremony_agent_status::CeremonyAgentStatus;
pub use ceremony_command::CeremonyCommand;
pub use ceremony_definition::CeremonyDefinition;
pub use ceremony_definition_draft::CeremonyDefinitionDraft;
pub use ceremony_event::CeremonyEvent;
pub use ceremony_event_reader::CeremonyEventReader;
pub use ceremony_evidence_pack::CeremonyEvidencePack;
pub use ceremony_instance::CeremonyInstance;
pub use ceremony_intervention::CeremonyIntervention;
pub use context_item::ContextItem;
pub use context_reference::ContextReference;
pub use context_summary::ContextSummary;
pub use council::Council;
pub use deliberation::Deliberation;
pub use deliberation_phase::DeliberationPhase;
pub use external_context::ExternalContextBundle;
pub use metric_family::MetricFamily;
pub use metric_sample::MetricSample;
pub use metrics_snapshot::MetricsSnapshot;
pub use proposal::Proposal;
pub use publication_outcome::PublicationOutcome;
pub use published_agentic_system::PublishedAgenticSystem;
pub use published_ceremony_definition::PublishedCeremonyDefinition;
pub use ranked_outcome::RankedOutcome;
pub use statistics::Statistics;
pub use task::Task;
pub use task_constraints::TaskConstraints;
pub use task_metadata::TaskMetadata;
pub use validation::ValidationOutcome;
pub use validator_report::ValidatorReport;

mod council_journal_event;
mod council_journal_record;
pub use council_journal_event::CouncilJournalEvent;
pub use council_journal_record::CouncilJournalRecord;

mod council_snapshot_provenance;
pub use council_snapshot_provenance::CouncilSnapshotProvenance;
mod agent_usage_kind;
pub use agent_usage_kind::AgentUsageKind;
