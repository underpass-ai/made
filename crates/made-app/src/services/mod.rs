//! Application services — compose one or more use cases.

mod auto_dispatch;
mod auto_dispatch_outcome;
mod conflict_policy;
mod loaded_session;
pub(crate) mod memory_scope_resolver;
mod retry_attempts;
pub(crate) mod session_facts;
mod session_memory_projection;
mod session_memory_recorder;
pub(crate) mod session_recall;
mod session_stream;

pub use auto_dispatch::AutoDispatchService;
pub use auto_dispatch_outcome::AutoDispatchOutcome;
pub use conflict_policy::ConflictPolicy;
pub use loaded_session::LoadedSession;
pub use retry_attempts::RetryAttempts;
pub use session_memory_recorder::SessionMemoryRecorder;
pub use session_stream::SessionStream;
