//! Domain ports.
//!
//! Ports are narrow, segregated traits. Each one names exactly one
//! responsibility that the application layer requires from the outside
//! world (agents, message bus, clock, persistence, …). Adapters in
//! `made-adapters` implement these traits.
//!
//! Hexagonal discipline:
//!
//! - Dependency direction is **adapters → app → core**. Ports live in
//!   core and import nothing from app or adapters.
//! - All ports return [`crate::DomainError`] so the application layer
//!   never leaks adapter-shaped errors (I/O, wire, parsing) upward.
//! - Segregation follows ISP: no port has more than one reason to
//!   change.

mod agent;
mod agent_descriptor;
mod agent_factory;
mod agent_registry;
mod agent_resolver;
mod audit_journal;
mod ceremony_definition_publication;
mod ceremony_definition_repository;
mod ceremony_definition_source;
mod ceremony_evidence_request;
mod ceremony_evidence_source;
mod ceremony_instance_repository;
mod ceremony_step_handler;
mod ceremony_step_handler_request;
mod ceremony_transcript_store;
mod ceremony_unit_of_work;
mod clock;
mod contract_registry;
mod council_registry;
mod critique;
mod deliberation_observer;
mod deliberation_repository;
mod domain_event;
mod draft_request;
mod evidence_support_judge;
mod executor;
mod memory_reader;
mod memory_recollection;
mod memory_write_outcome;
mod memory_writer;
mod messaging;
mod metrics_recorder;
mod noop_ceremony_transcript_store;
mod noop_metrics_recorder;
mod null_observer;
mod outbox;
mod outbox_transport;
mod revision;
mod scoring;
mod statistics;
mod subscription_handler;
mod validator;

pub use agent::AgentPort;
pub use agent_descriptor::AgentDescriptor;
pub use agent_factory::AgentFactoryPort;
pub use agent_registry::AgentRegistryPort;
pub use agent_resolver::AgentResolverPort;
pub use audit_journal::AuditJournalPort;
pub use ceremony_transcript_store::CeremonyTranscriptStorePort;

pub use ceremony_definition_publication::CeremonyDefinitionPublicationPort;
pub use ceremony_definition_repository::CeremonyDefinitionRepositoryPort;
pub use ceremony_definition_source::CeremonyDefinitionSourcePort;
pub use ceremony_evidence_request::CeremonyEvidenceRequest;
pub use ceremony_evidence_source::CeremonyEvidenceSourcePort;
pub use ceremony_instance_repository::CeremonyInstanceRepositoryPort;
pub use ceremony_step_handler::CeremonyStepHandlerPort;
pub use ceremony_step_handler_request::CeremonyStepHandlerRequest;
/// Former name of [`CeremonyTranscriptStorePort`].
///
/// Kept so a host can move at its own pace rather than in lockstep
/// with this repository. Due for removal before the first public tag —
/// a compatibility alias that outlives its migration is just a second
/// name for the same thing.
#[deprecated(
    since = "0.1.0",
    note = "renamed to CeremonyTranscriptStorePort: the port appends and replays a transcript, nothing more"
)]
pub use ceremony_transcript_store::CeremonyTranscriptStorePort as CeremonyContextStorePort;
pub use ceremony_unit_of_work::CeremonyUnitOfWorkPort;
pub use clock::ClockPort;
pub use contract_registry::ContractRegistryPort;
pub use council_registry::CouncilRegistryPort;
pub use critique::Critique;
pub use deliberation_observer::DeliberationObserverPort;
pub use deliberation_repository::DeliberationRepositoryPort;
pub use domain_event::DomainEvent;
pub use draft_request::DraftRequest;
pub use evidence_support_judge::EvidenceSupportJudgePort;
pub use executor::ExecutorPort;
pub use memory_reader::MemoryReaderPort;
pub use memory_recollection::MemoryRecollection;
pub use memory_write_outcome::MemoryWriteOutcome;
pub use memory_writer::MemoryWriterPort;
pub use messaging::MessagingPort;
pub use metrics_recorder::MetricsRecorderPort;
pub use noop_ceremony_transcript_store::NoopCeremonyTranscriptStore;
pub use noop_metrics_recorder::NoopMetricsRecorder;
pub use null_observer::NullObserver;
pub use outbox::OutboxPort;
pub use outbox_transport::OutboxTransportPort;
pub use revision::Revision;
pub use scoring::ScoringPort;
pub use statistics::StatisticsPort;
pub use subscription_handler::SubscriptionHandler;
pub use validator::ValidatorPort;
