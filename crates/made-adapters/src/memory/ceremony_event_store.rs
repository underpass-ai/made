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
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{
    seal_continuation, AppendOutcome, CeremonyEventStorePort, CeremonySnapshot,
    CeremonySnapshotStorePort, PositionedRecord,
};
use made_core::value_objects::{
    AuditSequence, CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion,
};
use tokio::sync::RwLock;

/// Where in which stream the record at a global position lives. The
/// record itself stays in its stream: the log is an index, not a copy.
type LogEntry = (GlobalPosition, CeremonyId, AuditSequence);

#[derive(Debug, Default)]
struct EventStoreState {
    streams: BTreeMap<CeremonyId, Vec<AuditRecord>>,
    log: Vec<LogEntry>,
    snapshots: BTreeMap<CeremonyId, BTreeMap<StreamVersion, CeremonyInstance>>,
}

impl EventStoreState {
    fn stream(&self, stream: &CeremonyId) -> &[AuditRecord] {
        self.streams.get(stream).map_or(&[], Vec::as_slice)
    }

    fn next_position(&self) -> GlobalPosition {
        self.log
            .last()
            .map_or(GlobalPosition::FIRST, |(position, _, _)| position.next())
    }

    fn record_at(&self, entry: &LogEntry) -> Result<PositionedRecord, DomainError> {
        let (position, stream, sequence) = entry;
        let index = usize::try_from(sequence.value().saturating_sub(1)).ok();
        let record = index
            .and_then(|index| self.stream(stream).get(index))
            .ok_or(DomainError::InvariantViolated {
                reason: "in-memory event store: the global log points at a missing record",
            })?;
        Ok(PositionedRecord {
            position: *position,
            record: record.clone(),
        })
    }
}

/// Implements both [`CeremonyEventStorePort`] and
/// [`CeremonySnapshotStorePort`] over the same state, for tests and
/// single-process hosts.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCeremonyEventStore {
    inner: Arc<RwLock<EventStoreState>>,
}

impl InMemoryCeremonyEventStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CeremonyEventStorePort for InMemoryCeremonyEventStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let mut state = self.inner.write().await;

        let existing = state.stream(stream);
        let actual = version_of(existing);
        if actual != expected {
            return Ok(AppendOutcome::Conflict { expected, actual });
        }
        // Sealed before anything is touched: a refused batch leaves the
        // stream and the log exactly as they were.
        let sealed = seal_continuation(stream, existing, facts)?;

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

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
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
        limit: CeremonyEventPageLimit,
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

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        Ok(version_of(self.inner.read().await.stream(stream)))
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        Ok(self.inner.read().await.streams.keys().cloned().collect())
    }
}

#[async_trait]
impl CeremonySnapshotStorePort for InMemoryCeremonyEventStore {
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError> {
        self.inner
            .write()
            .await
            .snapshots
            .entry(snapshot.instance.id().clone())
            .or_default()
            .insert(snapshot.version, snapshot.instance);
        Ok(())
    }

    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .snapshots
            .get(stream)
            .and_then(BTreeMap::last_key_value)
            .map(|(version, instance)| CeremonySnapshot {
                version: *version,
                instance: instance.clone(),
            }))
    }

    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError> {
        self.inner.write().await.snapshots.remove(stream);
        Ok(())
    }
}

fn version_of(records: &[AuditRecord]) -> StreamVersion {
    records.last().map_or(StreamVersion::EMPTY, |record| {
        StreamVersion::from_sequence(record.sequence())
    })
}
