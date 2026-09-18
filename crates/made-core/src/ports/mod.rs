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
mod append_outcome;
mod budget_append_outcome;
mod budget_ledger_snapshot;
mod budget_ledger_store;
mod budget_reservation_page;
mod ceremony_definition_publication;
mod ceremony_definition_repository;
mod ceremony_definition_source;
mod ceremony_event_cursor;
mod ceremony_event_store;
mod ceremony_event_subscriber;
mod ceremony_event_transport;
mod ceremony_evidence_request;
mod ceremony_evidence_source;
mod ceremony_execution_connector;
mod ceremony_execution_connector_outcome;
mod ceremony_execution_observation;
mod ceremony_execution_request;
mod ceremony_progress_notifier;
mod ceremony_progress_subscription;
mod ceremony_snapshot;
mod ceremony_snapshot_store;
mod ceremony_step_handler;
mod ceremony_step_handler_request;
mod clock;
mod contract_registry;
mod council_registry;
mod critique;
mod deliberation_observer;
mod deliberation_repository;
mod domain_event;
mod draft_request;
mod evidence_support_judge;
mod execution_receipt_store;
mod execution_recovery_page;
mod executor;
mod legacy_ceremony_snapshot;
mod legacy_ceremony_snapshot_source;
mod memory_reader;
mod memory_recollection;
mod memory_write_outcome;
mod memory_writer;
mod messaging;
mod metrics_recorder;
mod metrics_snapshot;
mod noop_ceremony_event_subscriber;
mod noop_metrics_recorder;
mod noop_metrics_snapshot;
mod null_observer;
mod positioned_record;
mod record_execution_intent_outcome;
mod record_execution_receipt_outcome;
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
pub use append_outcome::AppendOutcome;
pub use budget_append_outcome::BudgetAppendOutcome;
pub use budget_ledger_snapshot::BudgetLedgerSnapshot;
pub use budget_ledger_store::BudgetLedgerStorePort;
pub use budget_reservation_page::BudgetReservationPage;

pub use ceremony_definition_publication::CeremonyDefinitionPublicationPort;
pub use ceremony_definition_repository::CeremonyDefinitionRepositoryPort;
pub use ceremony_definition_source::CeremonyDefinitionSourcePort;
pub use ceremony_event_cursor::CeremonyEventCursorPort;
pub use ceremony_event_store::{seal_continuation, CeremonyEventStorePort};
pub use ceremony_event_subscriber::CeremonyEventSubscriberPort;
pub use ceremony_event_transport::CeremonyEventTransportPort;
pub use ceremony_evidence_request::CeremonyEvidenceRequest;
pub use ceremony_evidence_source::CeremonyEvidenceSourcePort;
pub use ceremony_execution_connector::CeremonyExecutionConnectorPort;
pub use ceremony_execution_connector_outcome::CeremonyExecutionConnectorOutcome;
pub use ceremony_execution_observation::CeremonyExecutionObservation;
pub use ceremony_execution_request::CeremonyExecutionRequest;
pub use ceremony_progress_notifier::CeremonyProgressNotifierPort;
pub use ceremony_progress_subscription::CeremonyProgressSubscriptionPort;
pub use ceremony_snapshot::CeremonySnapshot;
pub use ceremony_snapshot_store::CeremonySnapshotStorePort;
pub use ceremony_step_handler::CeremonyStepHandlerPort;
pub use ceremony_step_handler_request::CeremonyStepHandlerRequest;
pub use clock::ClockPort;
pub use contract_registry::ContractRegistryPort;
pub use council_registry::CouncilRegistryPort;
pub use critique::Critique;
pub use deliberation_observer::DeliberationObserverPort;
pub use deliberation_repository::DeliberationRepositoryPort;
pub use domain_event::DomainEvent;
pub use draft_request::DraftRequest;
pub use evidence_support_judge::EvidenceSupportJudgePort;
pub use execution_receipt_store::ExecutionReceiptStorePort;
pub use execution_recovery_page::ExecutionRecoveryPage;
pub use executor::ExecutorPort;
pub use legacy_ceremony_snapshot::LegacyCeremonySnapshot;
pub use legacy_ceremony_snapshot_source::LegacyCeremonySnapshotSourcePort;
pub use memory_reader::MemoryReaderPort;
pub use memory_recollection::MemoryRecollection;
pub use memory_write_outcome::MemoryWriteOutcome;
pub use memory_writer::MemoryWriterPort;
pub use messaging::MessagingPort;
pub use metrics_recorder::MetricsRecorderPort;
pub use metrics_snapshot::MetricsSnapshotPort;
pub use noop_ceremony_event_subscriber::NoopCeremonyEventSubscriber;
pub use noop_metrics_recorder::NoopMetricsRecorder;
pub use noop_metrics_snapshot::NoopMetricsSnapshot;
pub use null_observer::NullObserver;
pub use positioned_record::PositionedRecord;
pub use record_execution_intent_outcome::RecordExecutionIntentOutcome;
pub use record_execution_receipt_outcome::RecordExecutionReceiptOutcome;
pub use revision::Revision;
pub use scoring::ScoringPort;
pub use statistics::StatisticsPort;
pub use subscription_handler::SubscriptionHandler;
pub use validator::ValidatorPort;
