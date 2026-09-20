//! Canonical embedded persistence on SQLite.
//!
//! WAL mode lets several agent hosts share one durable store while a single
//! transaction spans a stream, its place in the global order and its
//! snapshots.

pub(crate) mod error;
mod keys;
mod stored_authorization_policy_state;
mod stored_ceremony;
mod stored_publication;

pub use authorization_policy_store::SqliteAuthorizationPolicyStore;
pub use budget_ledger_store::SqliteBudgetLedgerStore;
pub use ceremony_store::SqliteCeremonyStore;
pub use session_memory::SqliteSessionMemory;
pub(in crate::sqlite) use stored_authorization_policy_state::StoredAuthorizationPolicyState;
pub(in crate::sqlite) use stored_ceremony::StoredCeremony;
pub(in crate::sqlite) use stored_publication::StoredPublication;

mod authorization_policy_store;
mod budget_ledger_store;
mod ceremony_store;
mod session_memory;

mod council_store;
pub use council_store::SqliteCouncilStore;
mod council_registry;
pub use council_registry::SqliteCouncilRegistry;
mod contract_registry;
pub use contract_registry::SqliteContractRegistry;
mod deliberation_repository;
pub use deliberation_repository::SqliteDeliberationRepository;
mod council_statistics;
pub use council_statistics::SqliteCouncilStatistics;

mod agent_registry;
pub use agent_registry::SqliteAgentRegistry;

mod council_journal;
pub use council_journal::SqliteCouncilJournal;

mod council_snapshot;
pub use council_snapshot::SqliteCouncilSnapshot;

mod host_delivery_ledger;
mod host_delivery_scan;
mod integrator_bindings;
pub use host_delivery_ledger::SqliteHostDeliveryLedger;
pub use integrator_bindings::SqliteIntegratorBindings;
