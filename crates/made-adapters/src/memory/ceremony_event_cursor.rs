use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventCursorPort;
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventCursorLease,
    CeremonyEventCursorLeaseId, CeremonyEventQuarantineReason, DurationMs, GlobalPosition,
    QuarantinedCeremonyEvent,
};
use time::OffsetDateTime;
use tokio::sync::RwLock;

#[derive(Debug, Default)]
struct CursorState {
    acknowledged_through: Option<GlobalPosition>,
    attempt: CeremonyEventCursorAttempt,
    lease: Option<CeremonyEventCursorLease>,
    quarantined: Vec<QuarantinedCeremonyEvent>,
}

/// Process-local implementation of durable-cursor semantics.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCeremonyEventCursor {
    inner: Arc<RwLock<BTreeMap<CeremonyEventConsumer, CursorState>>>,
}

impl InMemoryCeremonyEventCursor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn require_lease<'a>(
        state: &'a mut CursorState,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
    ) -> Result<&'a mut CursorState, DomainError> {
        let matches = state
            .lease
            .as_ref()
            .is_some_and(|stored| stored.lease_id() == lease.lease_id());
        if !matches || position != lease.next_position() {
            return Err(DomainError::Conflict {
                what: "ceremony_event_cursor",
            });
        }
        Ok(state)
    }
}

#[async_trait]
impl CeremonyEventCursorPort for InMemoryCeremonyEventCursor {
    async fn position(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(consumer)
            .and_then(|state| state.acknowledged_through))
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(consumer.clone()).or_default();
        if state
            .lease
            .as_ref()
            .is_some_and(|lease| lease.leased_until() > now)
        {
            return Ok(None);
        }
        let until = now + Duration::from_millis(duration.get());
        let lease = CeremonyEventCursorLease::new(
            consumer.clone(),
            lease_id,
            state.acknowledged_through,
            state.attempt,
            until,
        );
        state.lease = Some(lease.clone());
        Ok(Some(lease))
    }

    async fn acknowledge(
        &self,
        consumer: &CeremonyEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(consumer.clone()).or_default();
        if state
            .acknowledged_through
            .is_some_and(|current| through <= current)
        {
            return Ok(());
        }
        if state.lease.is_some() {
            return Err(DomainError::Conflict {
                what: "ceremony_event_cursor",
            });
        }
        state.acknowledged_through = Some(through);
        state.attempt = CeremonyEventCursorAttempt::NONE;
        Ok(())
    }

    async fn acknowledge_lease(
        &self,
        lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(lease.consumer().clone()).or_default();
        let state = Self::require_lease(state, lease, through)?;
        state.acknowledged_through = Some(through);
        state.attempt = CeremonyEventCursorAttempt::NONE;
        state.lease = None;
        Ok(())
    }

    async fn mark_failed(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(lease.consumer().clone()).or_default();
        let state = Self::require_lease(state, lease, position)?;
        state.attempt = state.attempt.next();
        state.lease = None;
        Ok(())
    }

    async fn quarantine(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        reason: CeremonyEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(lease.consumer().clone()).or_default();
        let state = Self::require_lease(state, lease, position)?;
        state.quarantined.push(QuarantinedCeremonyEvent::new(
            lease.consumer().clone(),
            position,
            state.attempt,
            reason,
            now,
        ));
        state.acknowledged_through = Some(position);
        state.attempt = CeremonyEventCursorAttempt::NONE;
        state.lease = None;
        Ok(())
    }

    async fn release(&self, lease: &CeremonyEventCursorLease) -> Result<(), DomainError> {
        let mut cursors = self.inner.write().await;
        let state = cursors.entry(lease.consumer().clone()).or_default();
        let matches = state
            .lease
            .as_ref()
            .is_some_and(|stored| stored.lease_id() == lease.lease_id());
        if !matches {
            return Err(DomainError::Conflict {
                what: "ceremony_event_cursor",
            });
        }
        state.lease = None;
        Ok(())
    }

    async fn quarantined(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(consumer)
            .map_or_else(Vec::new, |state| state.quarantined.clone()))
    }
}
