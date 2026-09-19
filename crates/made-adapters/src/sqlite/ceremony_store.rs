//! [`SqliteCeremonyStore`] — the embedded durable store.
//!
//! The event streams, their global log and the snapshots live in tables
//! of one database, so an append lands its three tables in one write
//! transaction. Two tables beside them — `ceremony_instances` and
//! `audit_journal` — are what a pre-stream store held and are read-only
//! provenance now. Synchronous engine work always runs on Tokio's
//! blocking pool.

use std::sync::Arc;

use made_core::error::DomainError;
use serde::{Deserialize, Serialize};

use crate::engine::Engine;

use super::error::{encoding_failure, join_failure};

mod definition_publication;
mod event_cursor;
mod event_store;
mod execution_receipt_store;
mod instance_index;
mod legacy_snapshot_source;
#[cfg(test)]
mod legacy_store_fixture;
mod lifecycle;
mod snapshot_store;
mod stored_cursor;
mod stored_event;
mod stored_snapshot;
mod stored_snapshot_wire;

#[derive(Debug, Clone)]
pub struct SqliteCeremonyStore {
    pub(super) engine: Arc<dyn Engine>,
}

impl SqliteCeremonyStore {
    async fn blocking<T, F>(&self, op: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, op))?
    }
}

pub(super) fn encode<T: Serialize>(value: &T, op: &'static str) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(value).map_err(|error| encoding_failure(&error, op))
}

pub(super) fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    op: &'static str,
) -> Result<T, DomainError> {
    serde_json::from_slice(bytes).map_err(|error| encoding_failure(&error, op))
}
