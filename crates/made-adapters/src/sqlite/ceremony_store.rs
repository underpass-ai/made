//! [`SqliteCeremonyStore`] — the embedded durable store.
//!
//! Ceremony state, the audit journal and the outbox live in tables of one
//! database, so a commit that touches all three is one write transaction;
//! the event streams, their global log and the snapshots share it, and an
//! append lands its three tables the same way. Synchronous engine work
//! always runs on Tokio's blocking pool.

use std::sync::Arc;

use made_core::error::DomainError;
use serde::{Deserialize, Serialize};

use crate::engine::Engine;

use super::error::{encoding_failure, join_failure};

mod audit_journal;
mod ceremony_unit_of_work;
mod definition_publication;
mod event_store;
mod instance_repository;
mod lifecycle;
mod outbox;
mod snapshot_store;
mod stored_event;
mod stored_outbox_message;
mod stored_snapshot;

#[derive(Debug, Clone)]
pub struct SqliteCeremonyStore {
    engine: Arc<dyn Engine>,
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
