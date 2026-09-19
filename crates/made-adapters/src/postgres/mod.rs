//! Postgres-backed adapters.
//!
//! Opt-in via the `postgres` Cargo feature. Ships a pool builder, a
//! migration runner, and concrete implementations of the persistence
//! ports. The schema (see `migrations/postgres/`) stays minimal: the
//! full domain aggregate is stored as JSONB with a few indexable
//! projections, so no wire-format-shaped table grows with provider
//! vocabulary.
//!
//! Nothing here leaks sqlx types out of the module: callers outside
//! only ever see domain ports and a `PostgresPool` handle.

mod agent_registry;
mod artifact_blob_queries;
mod artifact_store;
mod budget_ledger_store;
mod ceremony_definition_publication;
mod ceremony_event_cursor;
mod ceremony_event_store;
mod ceremony_snapshot_store;
mod ceremony_store;
mod council_registry;
mod deliberation_repository;
mod error;
mod execution_receipt_store;
mod pool;
mod postgres_config;
mod postgres_pool_error;
mod postgres_session_memory;
mod postgres_stored_cursor;
mod statistics;

pub use agent_registry::PostgresAgentRegistry;
pub use artifact_store::PostgresArtifactStore;
pub use ceremony_store::PostgresCeremonyStore;
pub use council_registry::PostgresCouncilRegistry;
pub use deliberation_repository::PostgresDeliberationRepository;
pub use pool::PostgresPool;
pub use postgres_config::PostgresConfig;
pub use postgres_pool_error::PostgresPoolError;
pub use postgres_session_memory::PostgresSessionMemory;
pub use statistics::PostgresStatistics;

mod council_journal;
mod council_journal_store;
pub use council_journal::PostgresCouncilJournal;

mod contract_registry;
pub use contract_registry::PostgresContractRegistry;

mod council_snapshot;
mod council_snapshot_write;
pub use council_snapshot::PostgresCouncilSnapshot;
