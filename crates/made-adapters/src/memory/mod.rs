//! In-memory adapters backing the domain ports that hold state.
//!
//! These are the simplest possible implementations: data lives in a
//! mutex-guarded map inside an [`Arc`]. They are safe to share across
//! tasks and are production-safe for single-replica deployments; for
//! multi-replica use cases, swap them for a persistent adapter.

mod agent_registry;
mod ceremony_definition_publications;
mod ceremony_definition_repository;
mod ceremony_event_cursor;
mod ceremony_event_store;
mod contract_registry;
mod council_registry;
mod deliberation_repository;
mod forgetful_memory;
mod in_memory_message;
mod messaging;
mod session_memory;
mod statistics;

pub use agent_registry::InMemoryAgentRegistry;
pub use ceremony_definition_publications::InMemoryCeremonyDefinitionPublications;
pub use ceremony_definition_repository::InMemoryCeremonyDefinitionRepository;
pub use ceremony_event_cursor::InMemoryCeremonyEventCursor;
pub use ceremony_event_store::InMemoryCeremonyEventStore;
pub use contract_registry::InMemoryContractRegistry;
pub use council_registry::InMemoryCouncilRegistry;
pub use deliberation_repository::InMemoryDeliberationRepository;
pub use forgetful_memory::ForgetfulMemory;
pub use in_memory_message::InMemoryMessage;
pub use messaging::InMemoryMessaging;
pub use session_memory::InProcessSessionMemory;
pub use statistics::InMemoryStatistics;
