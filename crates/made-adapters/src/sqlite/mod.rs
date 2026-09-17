//! Canonical embedded persistence on SQLite.
//!
//! WAL mode lets several agent hosts share one durable store while a single
//! transaction spans a stream, its place in the global order and its
//! snapshots.

pub(crate) mod error;
mod keys;
mod stored_ceremony;
mod stored_publication;

pub use ceremony_store::SqliteCeremonyStore;
pub use session_memory::SqliteSessionMemory;
pub(in crate::sqlite) use stored_ceremony::StoredCeremony;
pub(in crate::sqlite) use stored_publication::StoredPublication;

mod ceremony_store;
mod session_memory;
