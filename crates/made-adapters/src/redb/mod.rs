//! Embedded persistence on redb.
//!
//! The store the embedded distribution uses: pure Rust, no non-optional
//! dependencies and no C toolchain, so an embedded host stays free of
//! system dependencies. One write transaction spans several tables,
//! which is what the ceremony unit of work needs.
//!
//! redb takes an exclusive lock on its file and serves a single
//! process. That is right for an embedded host and wrong for replicas.

mod conversion_receipt;
pub(crate) mod error;
mod keys;
mod legacy_definition_binding;
mod legacy_instance_migration;
mod legacy_publication_migration;
mod legacy_state_migration_receipt;
mod legacy_state_migrator;
mod stored_ceremony;
mod stored_publication;

pub use ceremony_store::RedbCeremonyStore;
pub use conversion_receipt::ConversionReceipt;
pub use legacy_state_migration_receipt::LegacyStateMigrationReceipt;
pub(in crate::redb) use stored_ceremony::StoredCeremony;
pub(in crate::redb) use stored_publication::StoredPublication;

mod ceremony_store;
