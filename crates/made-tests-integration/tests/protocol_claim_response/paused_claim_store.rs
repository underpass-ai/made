use async_trait::async_trait;
use made_adapters::memory::InMemoryProtocolEventStore;
use made_core::entities::{AuditFact, AuditRecord, ProtocolEvent};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, PositionedRecord, ProtocolEventStorePort, ProtocolInstanceIdPage,
    ProtocolInstanceIndexPort, ProtocolSnapshot, ProtocolSnapshotStorePort,
};
use made_core::value_objects::{
    AuthorizationEvidence, GlobalPosition, ProtocolEventPageLimit, ProtocolId, ProtocolIdPrefix,
    ProtocolInstancePageLimit, StreamVersion,
};
use tokio::sync::Notify;

/// Holds A's append response after durability, letting B reclaim before A renders.
#[derive(Default)]
pub(super) struct PausedClaimStore {
    inner: InMemoryProtocolEventStore,
    pub(super) committed: Notify,
    pub(super) resume: Notify,
}

#[async_trait]
impl ProtocolEventStorePort for PausedClaimStore {
    async fn append(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let first_claim = facts.iter().any(|fact| {
            matches!(&fact.event, ProtocolEvent::StepStarted(event)
                if event.lease.idempotency_key().as_str() == "worker-a")
        });
        let outcome = self.inner.append(stream, expected, facts).await?;
        if first_claim && matches!(outcome, AppendOutcome::Appended { .. }) {
            self.committed.notify_one();
            self.resume.notified().await;
        }
        Ok(outcome)
    }

    async fn append_authorized(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
        authorization: AuthorizationEvidence,
    ) -> Result<AppendOutcome, DomainError> {
        let first_claim = facts.iter().any(|fact| {
            matches!(&fact.event, ProtocolEvent::StepStarted(event)
                if event.lease.idempotency_key().as_str() == "worker-a")
        });
        let outcome = self
            .inner
            .append_authorized(stream, expected, facts, authorization)
            .await?;
        if first_claim && matches!(outcome, AppendOutcome::Appended { .. }) {
            self.committed.notify_one();
            self.resume.notified().await;
        }
        Ok(outcome)
    }

    async fn read(
        &self,
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &ProtocolId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<ProtocolId>, DomainError> {
        self.inner.streams().await
    }
}

#[async_trait]
impl ProtocolSnapshotStorePort for PausedClaimStore {
    async fn save(&self, snapshot: ProtocolSnapshot) -> Result<(), DomainError> {
        self.inner.save(snapshot).await
    }

    async fn latest(&self, stream: &ProtocolId) -> Result<Option<ProtocolSnapshot>, DomainError> {
        self.inner.latest(stream).await
    }

    async fn forget(&self, stream: &ProtocolId) -> Result<(), DomainError> {
        self.inner.forget(stream).await
    }
}

#[async_trait]
impl ProtocolInstanceIndexPort for PausedClaimStore {
    async fn ids_after(
        &self,
        after: Option<&ProtocolId>,
        id_prefix: Option<&ProtocolIdPrefix>,
        limit: ProtocolInstancePageLimit,
    ) -> Result<ProtocolInstanceIdPage, DomainError> {
        self.inner.ids_after(after, id_prefix, limit).await
    }
}
