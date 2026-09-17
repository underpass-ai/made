use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord};
use made_core::error::DomainError;
use made_core::ports::{AppendOutcome, CeremonyEventStorePort, PositionedRecord};
use made_core::value_objects::{CeremonyId, GlobalPosition, StreamVersion};
use tokio::sync::Notify;

use crate::usecases::ceremony_test_support::EventStoreFake;

/// Captures one read, lets a writer append, then returns that bounded cut.
#[derive(Debug)]
pub(super) struct ReportCutStore {
    inner: Arc<EventStoreFake>,
    block_next_read: AtomicBool,
    reads: AtomicUsize,
    captured: Notify,
    released: Notify,
}

impl ReportCutStore {
    pub(super) fn new(inner: Arc<EventStoreFake>) -> Self {
        Self {
            inner,
            block_next_read: AtomicBool::new(true),
            reads: AtomicUsize::new(0),
            captured: Notify::new(),
            released: Notify::new(),
        }
    }

    pub(super) async fn wait_until_captured(&self) {
        self.captured.notified().await;
    }

    pub(super) fn release(&self) {
        self.released.notify_one();
    }

    pub(super) fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CeremonyEventStorePort for ReportCutStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let cut = self.inner.read(stream, after).await?;
        if self.block_next_read.swap(false, Ordering::SeqCst) {
            self.captured.notify_one();
            self.released.notified().await;
        }
        Ok(cut)
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: usize,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.inner.streams().await
    }
}
