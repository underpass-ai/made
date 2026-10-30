use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ProtocolEventCursorPort;
use made_core::value_objects::{
    DurationMs, GlobalPosition, ProtocolEventConsumer, ProtocolEventCursorAttempt,
    ProtocolEventCursorLease, ProtocolEventCursorLeaseId, ProtocolEventQuarantineReason,
    QuarantinedProtocolEvent,
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
impl ProtocolEventCursorPort for AttentionCursorFake {
    async fn position(
        &self,
        _consumer: &ProtocolEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        Ok(*self.acknowledged.lock().expect("the cursor is readable"))
    }

    async fn lease(
        &self,
        consumer: &ProtocolEventConsumer,
        lease_id: ProtocolEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<ProtocolEventCursorLease>, DomainError> {
        Ok(Some(ProtocolEventCursorLease::new(
            consumer.clone(),
            lease_id,
            *self.acknowledged.lock().expect("the cursor is readable"),
            ProtocolEventCursorAttempt::NONE,
            now + time::Duration::milliseconds(duration.get() as i64),
        )))
    }

    async fn acknowledge(
        &self,
        _consumer: &ProtocolEventConsumer,
        _through: GlobalPosition,
    ) -> Result<(), DomainError> {
        unimplemented!("the projector acknowledges under its lease")
    }

    async fn acknowledge_lease(
        &self,
        _lease: &ProtocolEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(through);
        Ok(())
    }

    async fn mark_failed(
        &self,
        _lease: &ProtocolEventCursorLease,
        _position: GlobalPosition,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantine(
        &self,
        _lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
        _reason: ProtocolEventQuarantineReason,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        *self.acknowledged.lock().expect("the cursor is writable") = Some(position);
        Ok(())
    }

    async fn release(&self, _lease: &ProtocolEventCursorLease) -> Result<(), DomainError> {
        Ok(())
    }

    async fn quarantined(
        &self,
        _consumer: &ProtocolEventConsumer,
    ) -> Result<Vec<QuarantinedProtocolEvent>, DomainError> {
        Ok(Vec::new())
    }
}
