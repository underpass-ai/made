//! Storage-shape and decisions for host deliveries, shared by adapters.
//!
//! The records here are what memory, SQLite and Postgres all write, and
//! the decisions on them are what all three take. Duplicating either
//! would let two adapters pass the same contract suite while disagreeing
//! about when a lease has expired.

mod stored_host_delivery;
mod stored_integrator_binding;

pub(crate) use stored_host_delivery::StoredHostDelivery;
pub(crate) use stored_integrator_binding::StoredIntegratorBinding;
