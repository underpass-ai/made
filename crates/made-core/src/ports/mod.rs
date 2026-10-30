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
mod agentic_system_diagram;
mod agentic_system_diagram_rendering;
mod agentic_system_execution_creation;
mod agentic_system_execution_store;
mod agentic_system_execution_update;
mod agentic_system_page;
mod agentic_system_publication;
mod agentic_system_query;
mod agentic_system_repository;
mod agentic_system_save_outcome;
mod append_outcome;
mod artifact;
mod authorization_decision_page;
mod authorization_policy_append_outcome;
mod authorization_policy_snapshot;
mod authorization_policy_store;
mod authorization_scope_resolver;
mod budget_append_outcome;
mod budget_ledger_snapshot;
mod budget_ledger_store;
mod budget_reservation_page;
mod budget_reservation_planner;
mod clock;
mod contract_registry;
mod council_registry;
mod critique;
mod deliberation_observer;
mod deliberation_repository;
mod domain_event;
mod draft_request;
mod evidence_signer;
mod evidence_support_judge;
mod evidence_verifier;
mod execution_cancellation;
mod execution_profile_resolver;
mod execution_receipt_store;
mod execution_recovery_page;
mod executor;
mod legacy_protocol_snapshot;
mod legacy_protocol_snapshot_source;
mod memory_reader;
mod memory_recollection;
mod memory_write_outcome;
mod memory_writer;
mod messaging;
mod metrics_recorder;
mod metrics_snapshot;
mod noop_metrics_recorder;
mod noop_metrics_snapshot;
mod noop_protocol_event_subscriber;
mod null_observer;
mod positioned_record;
mod protocol_agent_activity_page;
mod protocol_agent_activity_query;
mod protocol_agent_activity_subscription;
mod protocol_agent_status;
mod protocol_agent_status_page;
mod protocol_agent_status_query;
mod protocol_definition_publication;
mod protocol_definition_repository;
mod protocol_definition_source;
mod protocol_event_cursor;
mod protocol_event_store;
mod protocol_event_subscriber;
mod protocol_event_transport;
mod protocol_evidence_request;
mod protocol_evidence_source;
mod protocol_execution_connector;
mod protocol_execution_connector_outcome;
mod protocol_execution_observation;
mod protocol_execution_request;
mod protocol_instance_id_page;
mod protocol_instance_index;
mod protocol_progress_notifier;
mod protocol_progress_subscription;
mod protocol_snapshot;
mod protocol_snapshot_store;
mod protocol_step_handler;
mod protocol_step_handler_request;
mod record_execution_intent_outcome;
mod record_execution_receipt_outcome;
mod recorded_activation;
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
pub use artifact::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactChunkPage, ArtifactIdempotencyKey,
    ArtifactPage, ArtifactPageLimit, ArtifactProtectionStatus, ArtifactReadCompletion,
    ArtifactRecord, ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactSnapshot,
    ArtifactStoreError, ArtifactStorePort, ArtifactTombstone, ArtifactUploadId,
    ArtifactUploadStatus, BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk,
    RestoreProtectionKey, TombstoneArtifact, ARTIFACT_DEFAULT_CHUNK_BYTES, ARTIFACT_MAX_BYTES,
    ARTIFACT_MAX_CHUNK_BYTES, ARTIFACT_MAX_PAGE_ITEMS,
};
pub use authorization_decision_page::AuthorizationDecisionPage;
pub use authorization_policy_append_outcome::AuthorizationPolicyAppendOutcome;
pub use authorization_policy_snapshot::AuthorizationPolicySnapshot;
pub use authorization_policy_store::AuthorizationPolicyStorePort;
pub use authorization_scope_resolver::AuthorizationScopeResolverPort;
pub use budget_append_outcome::BudgetAppendOutcome;
pub use budget_ledger_snapshot::BudgetLedgerSnapshot;
pub use budget_ledger_store::BudgetLedgerStorePort;
pub use budget_reservation_page::BudgetReservationPage;
pub use budget_reservation_planner::BudgetReservationPlannerPort;

pub use clock::ClockPort;
pub use contract_registry::ContractRegistryPort;
pub use council_registry::CouncilRegistryPort;
pub use critique::Critique;
pub use deliberation_observer::DeliberationObserverPort;
pub use deliberation_repository::DeliberationRepositoryPort;
pub use domain_event::DomainEvent;
pub use draft_request::DraftRequest;
pub use evidence_signer::EvidenceSignerPort;
pub use evidence_support_judge::EvidenceSupportJudgePort;
pub use evidence_verifier::EvidenceVerifierPort;
pub use execution_cancellation::ExecutionCancellation;
pub use execution_profile_resolver::ExecutionProfileResolverPort;
pub use execution_receipt_store::ExecutionReceiptStorePort;
pub use execution_recovery_page::ExecutionRecoveryPage;
pub use executor::ExecutorPort;
pub use legacy_protocol_snapshot::LegacyProtocolSnapshot;
pub use legacy_protocol_snapshot_source::LegacyProtocolSnapshotSourcePort;
pub use memory_reader::MemoryReaderPort;
pub use memory_recollection::MemoryRecollection;
pub use memory_write_outcome::MemoryWriteOutcome;
pub use memory_writer::MemoryWriterPort;
pub use messaging::MessagingPort;
pub use metrics_recorder::MetricsRecorderPort;
pub use metrics_snapshot::MetricsSnapshotPort;
pub use noop_metrics_recorder::NoopMetricsRecorder;
pub use noop_metrics_snapshot::NoopMetricsSnapshot;
pub use noop_protocol_event_subscriber::NoopProtocolEventSubscriber;
pub use null_observer::NullObserver;
pub use positioned_record::PositionedRecord;
pub use protocol_agent_activity_page::ProtocolAgentActivityPage;
pub use protocol_agent_activity_query::ProtocolAgentActivityQuery;
pub use protocol_agent_activity_subscription::ProtocolAgentActivitySubscriptionPort;
pub use protocol_agent_status::ProtocolAgentStatusPort;
pub use protocol_agent_status_page::ProtocolAgentStatusPage;
pub use protocol_agent_status_query::ProtocolAgentStatusQuery;
pub use protocol_definition_publication::ProtocolDefinitionPublicationPort;
pub use protocol_definition_repository::ProtocolDefinitionRepositoryPort;
pub use protocol_definition_source::ProtocolDefinitionSourcePort;
pub use protocol_event_cursor::ProtocolEventCursorPort;
pub use protocol_event_store::{seal_continuation, ProtocolEventStorePort};
pub use protocol_event_subscriber::ProtocolEventSubscriberPort;
pub use protocol_event_transport::ProtocolEventTransportPort;
pub use protocol_evidence_request::ProtocolEvidenceRequest;
pub use protocol_evidence_source::ProtocolEvidenceSourcePort;
pub use protocol_execution_connector::ProtocolExecutionConnectorPort;
pub use protocol_execution_connector_outcome::ProtocolExecutionConnectorOutcome;
pub use protocol_execution_observation::ProtocolExecutionObservation;
pub use protocol_execution_request::ProtocolExecutionRequest;
pub use protocol_instance_id_page::ProtocolInstanceIdPage;
pub use protocol_instance_index::ProtocolInstanceIndexPort;
pub use protocol_progress_notifier::ProtocolProgressNotifierPort;
pub use protocol_progress_subscription::ProtocolProgressSubscriptionPort;
pub use protocol_snapshot::ProtocolSnapshot;
pub use protocol_snapshot_store::ProtocolSnapshotStorePort;
pub use protocol_step_handler::ProtocolStepHandlerPort;
pub use protocol_step_handler_request::ProtocolStepHandlerRequest;
pub use record_execution_intent_outcome::RecordExecutionIntentOutcome;
pub use record_execution_receipt_outcome::RecordExecutionReceiptOutcome;
pub use recorded_activation::RecordedActivation;
pub use revision::Revision;
pub use scoring::ScoringPort;
pub use statistics::StatisticsPort;
pub use subscription_handler::SubscriptionHandler;
pub use validator::ValidatorPort;

mod council_journal;
pub use council_journal::CouncilJournalPort;

// Delivering work to a host: the ledger that holds it, the binding that
// says where it goes, and the adapter that wakes what is there.
mod ack_outcome;
mod bind_outcome;
mod bind_replacement;
mod delivery_failure_outcome;
mod enqueue_outcome;
mod host_activation;
mod host_activation_outcome;
mod host_delivery_filter;
mod host_delivery_ledger;
mod host_delivery_page;
mod host_delivery_page_limit;
mod host_delivery_query;
mod host_delivery_target_filter;
mod integrator_binding;
mod leased_delivery;
mod processed_outcome;
mod supersession_outcome;

pub use ack_outcome::AckOutcome;
pub use agentic_system_diagram::AgenticSystemDiagramPort;
pub use agentic_system_diagram_rendering::AgenticSystemDiagram;
pub use agentic_system_execution_creation::AgenticSystemExecutionCreation;
pub use agentic_system_execution_store::AgenticSystemExecutionStorePort;
pub use agentic_system_execution_update::AgenticSystemExecutionUpdate;
pub use agentic_system_page::AgenticSystemPage;
pub use agentic_system_publication::AgenticSystemPublicationPort;
pub use agentic_system_query::AgenticSystemQuery;
pub use agentic_system_repository::AgenticSystemRepositoryPort;
pub use agentic_system_save_outcome::AgenticSystemSaveOutcome;
pub use bind_outcome::BindOutcome;
pub use bind_replacement::BindReplacement;
pub use delivery_failure_outcome::DeliveryFailureOutcome;
pub use enqueue_outcome::EnqueueOutcome;
pub use host_activation::HostActivationPort;
pub use host_activation_outcome::HostActivationOutcome;
pub use host_delivery_filter::HostDeliveryFilter;
pub use host_delivery_ledger::HostDeliveryLedgerPort;
pub use host_delivery_page::HostDeliveryPage;
pub use host_delivery_page_limit::HostDeliveryPageLimit;
pub use host_delivery_query::HostDeliveryQuery;
pub use host_delivery_target_filter::HostDeliveryTargetFilter;
pub use integrator_binding::IntegratorBindingPort;
pub use leased_delivery::LeasedDelivery;
pub use processed_outcome::ProcessedOutcome;
pub use supersession_outcome::SupersessionOutcome;
