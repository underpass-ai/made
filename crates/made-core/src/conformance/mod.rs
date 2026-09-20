//! Contract suites the engine ships so hosts can prove their adapters.
//!
//! The engine cedes storage, not the contract. A suite here is the
//! difference between a port a host implements and a port a host can be
//! shown to have implemented.
//!
//! Enabled by the `conformance` feature so the suites never enter a
//! production build, while staying available to any host, inside this
//! repository or outside it.

mod agentic_system_execution_store_conformance;
mod agentic_system_fixtures;
mod agentic_system_publication_conformance;
mod agentic_system_repository_conformance;
mod budget_ledger_store_conformance;
mod ceremony_definition_publication_conformance;
mod ceremony_event_cursor_conformance;
mod ceremony_event_store_conformance;
mod ceremony_snapshot_store_conformance;
mod conformance_failure;
mod conformance_fixtures;
mod host_delivery_fixtures;
mod host_delivery_ledger_conformance;
mod integrator_binding_conformance;
mod memory_conformance;
mod memory_conformance_capabilities;
mod memory_conformance_failure;

pub use agentic_system_execution_store_conformance::AgenticSystemExecutionStoreConformance;
pub use agentic_system_publication_conformance::AgenticSystemPublicationConformance;
pub use agentic_system_repository_conformance::AgenticSystemRepositoryConformance;
pub use budget_ledger_store_conformance::BudgetLedgerStoreConformance;
pub use ceremony_definition_publication_conformance::CeremonyDefinitionPublicationConformance;
pub use ceremony_event_cursor_conformance::CeremonyEventCursorConformance;
pub use ceremony_event_store_conformance::CeremonyEventStoreConformance;
pub use ceremony_snapshot_store_conformance::CeremonySnapshotStoreConformance;
pub use conformance_failure::ConformanceFailure;
pub use host_delivery_ledger_conformance::HostDeliveryLedgerConformance;
pub use integrator_binding_conformance::IntegratorBindingConformance;
pub use memory_conformance::MemoryConformance;
pub use memory_conformance_failure::MemoryConformanceFailure;
