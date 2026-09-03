//! Domain entities and aggregates.
//!
//! Entities have identity that persists across state changes. Aggregate
//! roots own invariants spanning multiple objects; state transitions
//! happen through their methods, not by mutating fields directly.

mod audit_chain;
mod audit_fact;
mod audit_record;
mod ceremony_commit;
mod ceremony_definition;
mod ceremony_definition_analysis;
mod ceremony_definition_draft;
mod ceremony_evidence_pack;
mod ceremony_instance;
mod ceremony_intervention;
mod commit_outcome;
mod context_item;
mod context_reference;
mod context_summary;
mod council;
mod deliberation;
mod deliberation_phase;
mod external_context;
mod external_context_validation;
mod proposal;
mod publication_outcome;
mod published_ceremony_definition;
mod ranked_outcome;
mod statistics;
mod task;
mod task_constraints;
mod task_metadata;
mod validation;
mod validator_report;

pub use audit_chain::AuditChain;
pub use audit_fact::AuditFact;
pub use audit_record::{AuditRecord, AUDIT_RECORD_SCHEMA_VERSION};
pub use ceremony_commit::CeremonyCommit;
pub use ceremony_definition::CeremonyDefinition;
pub use ceremony_definition_draft::CeremonyDefinitionDraft;
pub use ceremony_evidence_pack::CeremonyEvidencePack;
pub use ceremony_instance::CeremonyInstance;
pub use ceremony_intervention::CeremonyIntervention;
pub use commit_outcome::CommitOutcome;
pub use context_item::ContextItem;
pub use context_reference::ContextReference;
pub use context_summary::ContextSummary;
pub use council::Council;
pub use deliberation::Deliberation;
pub use deliberation_phase::DeliberationPhase;
pub use external_context::ExternalContextBundle;
pub use proposal::Proposal;
pub use publication_outcome::PublicationOutcome;
pub use published_ceremony_definition::PublishedCeremonyDefinition;
pub use ranked_outcome::RankedOutcome;
pub use statistics::Statistics;
pub use task::Task;
pub use task_constraints::TaskConstraints;
pub use task_metadata::TaskMetadata;
pub use validation::ValidationOutcome;
pub use validator_report::ValidatorReport;
