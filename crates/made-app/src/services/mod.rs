//! Application services — compose one or more use cases.

mod authorization_operation_scope;
#[cfg(test)]
mod authorization_operation_scope_tests;
mod auto_dispatch;
mod auto_dispatch_outcome;
mod ceremony_event_fanout;
mod ceremony_event_publisher_subscriber;
mod ceremony_trace_scope;
pub(crate) mod ceremony_transcript_projection;
mod conflict_policy;
mod intervention_delivery_subscriber;
mod loaded_session;
pub(crate) mod memory_scope_resolver;
mod retry_attempts;
pub(crate) mod session_facts;
mod session_facts_interventions;
mod session_memory_projection;
mod session_memory_recorder;
pub(crate) mod session_recall;
mod session_stream;
pub(crate) mod succession_evidence;

pub(crate) use authorization_operation_scope::current_authorized_operation;
pub use authorization_operation_scope::AuthorizationOperationScope;
pub use auto_dispatch::AutoDispatchService;
pub use auto_dispatch_outcome::AutoDispatchOutcome;
pub use ceremony_event_fanout::CeremonyEventFanout;
pub use ceremony_event_publisher_subscriber::CeremonyEventPublisherSubscriber;
pub(crate) use ceremony_trace_scope::current_trace_context;
pub use ceremony_trace_scope::CeremonyTraceScope;
pub use conflict_policy::ConflictPolicy;
pub use intervention_delivery_subscriber::InterventionDeliverySubscriber;
pub use loaded_session::LoadedSession;
pub use retry_attempts::RetryAttempts;
pub use session_memory_recorder::SessionMemoryRecorder;
pub use session_stream::SessionStream;

mod council_journal_service;
pub use council_journal_service::CouncilJournalService;
