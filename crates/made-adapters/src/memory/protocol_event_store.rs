//! In-memory event store: streams, their global order and snapshots
//! under one lock.
//!
//! One lock, because an append is one claim about three things — the
//! stream grew, the global log grew by the same records, and the
//! expectation held — and a store that guarded each separately could
//! satisfy every property except the one that matters. Snapshots share
//! it for convenience only; they are a cache and nothing an append does
//! depends on them.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord};
use made_core::error::DomainError;
use made_core::ports::{
    seal_continuation, AppendOutcome, PositionedRecord, ProtocolEventStorePort,
    ProtocolInstanceIdPage, ProtocolInstanceIndexPort, ProtocolSnapshot, ProtocolSnapshotStorePort,
};
use made_core::value_objects::{
    AuthorizationEvidence, GlobalPosition, ProtocolEventPageLimit, ProtocolId, ProtocolIdPrefix,
    ProtocolInstancePageLimit, StreamVersion,
};
use tokio::sync::RwLock;

mod event_store_state;

use event_store_state::EventStoreState;

/// Implements both [`ProtocolEventStorePort`] and
/// [`ProtocolSnapshotStorePort`] over the same state, for tests and
/// single-process hosts.
#[derive(Debug, Default, Clone)]
pub struct InMemoryProtocolEventStore {
    inner: Arc<RwLock<EventStoreState>>,
}

impl InMemoryProtocolEventStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    async fn append_with_authorization(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
        authorization: Option<&AuthorizationEvidence>,
    ) -> Result<AppendOutcome, DomainError> {
        let mut state = self.inner.write().await;

        let existing = state.stream(stream);
        let actual = version_of(existing);
        if actual != expected {
            return Ok(AppendOutcome::Conflict { expected, actual });
        }
        let sealed = seal_continuation(stream, existing, facts, authorization)?;

        let first_position = state.next_position();
        let mut position = first_position;
        for record in &sealed {
            state
                .log
                .push((position, stream.clone(), record.sequence()));
            position = position.next();
        }
        let records = state.streams.entry(stream.clone()).or_default();
        records.extend(sealed.iter().cloned());
        let version = version_of(records);

        Ok(AppendOutcome::Appended {
            version,
            records: sealed,
            first_position,
        })
    }
}

#[async_trait]
impl ProtocolEventStorePort for InMemoryProtocolEventStore {
    async fn append(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.append_with_authorization(stream, expected, facts, None)
            .await
    }

    async fn append_authorized(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
        authorization: AuthorizationEvidence,
    ) -> Result<AppendOutcome, DomainError> {
        self.append_with_authorization(stream, expected, facts, Some(&authorization))
            .await
    }

    async fn read(
        &self,
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let state = self.inner.read().await;
        let skipped = usize::try_from(after.value()).unwrap_or(usize::MAX);
        Ok(state
            .stream(stream)
            .iter()
            .skip(skipped)
            .take(limit.value())
            .cloned()
            .collect())
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        let state = self.inner.read().await;
        // Positions are contiguous from 1, so the entry at index `i`
        // sits at position `i + 1`.
        let skipped = usize::try_from(from.value().saturating_sub(1)).unwrap_or(usize::MAX);
        state
            .log
            .iter()
            .skip(skipped)
            .take(limit.value())
            .map(|entry| state.record_at(entry))
            .collect()
    }

    async fn head(&self, stream: &ProtocolId) -> Result<StreamVersion, DomainError> {
        Ok(version_of(self.inner.read().await.stream(stream)))
    }

    async fn streams(&self) -> Result<Vec<ProtocolId>, DomainError> {
        Ok(self.inner.read().await.streams.keys().cloned().collect())
    }
}

#[async_trait]
impl ProtocolSnapshotStorePort for InMemoryProtocolEventStore {
    async fn save(&self, snapshot: ProtocolSnapshot) -> Result<(), DomainError> {
        self.inner
            .write()
            .await
            .snapshots
            .entry(snapshot.instance.id().clone())
            .or_default()
            .insert(snapshot.version, snapshot.instance);
        Ok(())
    }

    async fn latest(&self, stream: &ProtocolId) -> Result<Option<ProtocolSnapshot>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .snapshots
            .get(stream)
            .and_then(BTreeMap::last_key_value)
            .map(|(version, instance)| ProtocolSnapshot {
                version: *version,
                instance: instance.clone(),
            }))
    }

    async fn forget(&self, stream: &ProtocolId) -> Result<(), DomainError> {
        self.inner.write().await.snapshots.remove(stream);
        Ok(())
    }
}

#[async_trait]
impl ProtocolInstanceIndexPort for InMemoryProtocolEventStore {
    async fn ids_after(
        &self,
        after: Option<&ProtocolId>,
        id_prefix: Option<&ProtocolIdPrefix>,
        limit: ProtocolInstancePageLimit,
    ) -> Result<ProtocolInstanceIdPage, DomainError> {
        let state = self.inner.read().await;
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);
        let mut ids: Vec<_> = state
            .streams
            .range((lower, Bound::Unbounded))
            .map(|(id, _)| id)
            .filter(|id| id_prefix.is_none_or(|prefix| id.as_str().starts_with(prefix.as_str())))
            .take(limit.value() + 1)
            .cloned()
            .collect();
        let has_more = ids.len() > limit.value();
        ids.truncate(limit.value());
        Ok(ProtocolInstanceIdPage::new(ids, has_more))
    }
}

fn version_of(records: &[AuditRecord]) -> StreamVersion {
    records.last().map_or(StreamVersion::EMPTY, |record| {
        StreamVersion::from_sequence(record.sequence())
    })
}
