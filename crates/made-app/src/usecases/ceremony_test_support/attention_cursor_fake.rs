use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventCursorPort;
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventCursorLease,
    CeremonyEventCursorLeaseId, CeremonyEventQuarantineReason, DurationMs, GlobalPosition,
    QuarantinedCeremonyEvent,
};
use time::OffsetDateTime;

/// One consumer's place in the global feed, held in memory.
///
/// Every lease is granted: what these tests exercise is a projection
/// that runs on the read path, not two workers racing for a cursor.
#[derive(Default)]
pub(in crate::usecases) struct AttentionCursorFake {
    acknowledged: Mutex<Option<GlobalPosition>>,
}

#[async_trait]
impl CeremonyEventCursorPort for AttentionCursorFake {
    async fn position(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        Ok(*self.acknowledged.lock().expect("the cursor is readable"))
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError> {
        Ok(Some(CeremonyEventCursorLease::new(
            consumer.clone(),
            lease_id,
            *self.acknowledged.lock().expect("the cursor is readable"),
            CeremonyEventCursorAttempt::NONE,
            now + time::Duration::milliseconds(duration.get() as i64),
        )))
    }

    async fn acknowledge(
        &self,
        _consumer: &CeremonyEventConsumer,
        _through: GlobalPosition,
    ) -> Result<(), DomainError> {
        unimplemented!("the projector acknowledges under its lease")
    }

    async fn acknowledge_lease(
        &self,
        _lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(through);
        Ok(())
    }

    async fn mark_failed(
        &self,
        _lease: &CeremonyEventCursorLease,
        _position: GlobalPosition,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantine(
        &self,
        _lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        _reason: CeremonyEventQuarantineReason,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(position);
        Ok(())
    }

    async fn release(&self, _lease: &CeremonyEventCursorLease) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantined(
        &self,
        _consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError> {
        Ok(Vec::new())
    }
}
