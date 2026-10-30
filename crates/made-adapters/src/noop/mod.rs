//! No-op adapters.
//!
//! These implementations honour the port contract without performing
//! any externally-visible side effect. They are the safe defaults for
//! deployments that disable a subsystem (e.g. `nats_enabled=false`)
//! and are also used extensively in tests.

mod agent;
mod agent_factory;
mod executor;
mod messaging;
mod protocol_evidence_source;
mod protocol_step_handler;

pub use agent::NoopAgent;
pub use agent_factory::{NoopAgentFactory, NOOP_AGENT_KIND};
pub use executor::NoopExecutor;
pub use messaging::NoopMessaging;
pub use protocol_evidence_source::NoopProtocolEvidenceSource;
pub use protocol_step_handler::NoopProtocolStepHandler;
