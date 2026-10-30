//! Application services — compose one or more use cases.

pub mod attention;
mod authorization_operation_scope;
#[cfg(test)]
mod authorization_operation_scope_tests;
mod auto_dispatch;
mod auto_dispatch_outcome;
mod conflict_policy;
mod intervention_delivery_subscriber;
mod loaded_session;
pub(crate) mod memory_scope_resolver;
mod protocol_event_fanout;
mod protocol_event_publisher_subscriber;
mod protocol_trace_scope;
pub(crate) mod protocol_transcript_projection;
mod retry_attempts;
pub(crate) mod session_facts;
mod session_memory_projection;
mod session_memory_recorder;
pub(crate) mod session_recall;
mod session_stream;
pub(crate) mod succession_evidence;

pub(crate) use authorization_operation_scope::current_authorized_operation;
pub use authorization_operation_scope::AuthorizationOperationScope;
pub use auto_dispatch::AutoDispatchService;
pub use auto_dispatch_outcome::AutoDispatchOutcome;
pub use conflict_policy::ConflictPolicy;
pub use intervention_delivery_subscriber::InterventionDeliverySubscriber;
pub use loaded_session::LoadedSession;
pub use protocol_event_fanout::ProtocolEventFanout;
pub use protocol_event_publisher_subscriber::ProtocolEventPublisherSubscriber;
pub(crate) use protocol_trace_scope::current_trace_context;
pub use protocol_trace_scope::ProtocolTraceScope;
pub use retry_attempts::RetryAttempts;
pub use session_memory_recorder::SessionMemoryRecorder;
pub use session_stream::SessionStream;

mod council_journal_service;
pub use council_journal_service::CouncilJournalService;
