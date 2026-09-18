use async_trait::async_trait;
use made_adapters::memory::InMemoryCeremonyEventStore;
use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, CeremonyEventStorePort, CeremonySnapshot, CeremonySnapshotStorePort,
    PositionedRecord,
};
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion};
use tokio::sync::Notify;

/// Holds A's append response after durability, letting B reclaim before A renders.
#[derive(Default)]
pub(super) struct PausedClaimStore {
    inner: InMemoryCeremonyEventStore,
    pub(super) committed: Notify,
    pub(super) resume: Notify,
}

#[async_trait]
impl CeremonyEventStorePort for PausedClaimStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let first_claim = facts.iter().any(|fact| {
            matches!(&fact.event, CeremonyEvent::StepStarted(event)
                if event.lease.idempotency_key().as_str() == "worker-a")
        });
        let outcome = self.inner.append(stream, expected, facts).await?;
        if first_claim && matches!(outcome, AppendOutcome::Appended { .. }) {
            self.committed.notify_one();
            self.resume.notified().await;
        }
        Ok(outcome)
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
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

#[async_trait]
impl CeremonySnapshotStorePort for PausedClaimStore {
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError> {
        self.inner.save(snapshot).await
    }

    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError> {
        self.inner.latest(stream).await
    }

    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError> {
        self.inner.forget(stream).await
    }
}
