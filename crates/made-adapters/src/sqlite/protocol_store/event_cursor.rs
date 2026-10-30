use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ProtocolEventCursorPort;
use made_core::value_objects::{
    DurationMs, GlobalPosition, ProtocolEventConsumer, ProtocolEventCursorAttempt,
    ProtocolEventCursorLease, ProtocolEventCursorLeaseId, ProtocolEventQuarantineReason,
    QuarantinedProtocolEvent,
};
use time::OffsetDateTime;

use crate::engine::{Key, ReadTx, Table, WriteTx};

use super::stored_cursor::StoredCursor;
use super::{decode, encode, SqliteProtocolStore};

fn read_cursor(
    tx: &dyn ReadTx,
    consumer: &ProtocolEventConsumer,
) -> Result<StoredCursor, DomainError> {
    tx.get(Table::EventCursors, Key::Str(consumer.as_str()))?
        .map(|bytes| decode(&bytes, "decode protocol event cursor"))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn write_cursor(
    tx: &mut dyn WriteTx,
    consumer: &ProtocolEventConsumer,
    cursor: &StoredCursor,
) -> Result<(), DomainError> {
    tx.insert(
        Table::EventCursors,
        Key::Str(consumer.as_str()),
        &encode(cursor, "encode protocol event cursor")?,
    )
}

fn quarantine_key(consumer: &ProtocolEventConsumer, position: GlobalPosition) -> Vec<u8> {
    let mut key = consumer.as_str().as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(&position.value().to_be_bytes());
    key
}

fn quarantine_range(consumer: &ProtocolEventConsumer) -> (Vec<u8>, Vec<u8>) {
    let mut start = consumer.as_str().as_bytes().to_vec();
    start.push(0);
    start.extend_from_slice(&0_u64.to_be_bytes());
    let mut end = consumer.as_str().as_bytes().to_vec();
    end.push(0);
    end.extend_from_slice(&u64::MAX.to_be_bytes());
    (start, end)
}

fn require_lease(
    stored: &StoredCursor,
    lease: &ProtocolEventCursorLease,
    position: GlobalPosition,
) -> Result<(), DomainError> {
    let matches = stored
        .lease
        .as_ref()
        .is_some_and(|active| active.lease_id() == lease.lease_id());
    if !matches || position != lease.next_position() {
        return Err(DomainError::Conflict {
            what: "protocol_event_cursor",
        });
    }
    Ok(())
}

#[async_trait]
impl ProtocolEventCursorPort for SqliteProtocolStore {
    async fn position(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("read protocol event cursor", move |engine| {
            let tx = engine.begin_read()?;
            Ok(read_cursor(tx.as_ref(), &consumer)?.acknowledged_through)
        })
        .await
    }

    async fn lease(
        &self,
        consumer: &ProtocolEventConsumer,
        lease_id: ProtocolEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<ProtocolEventCursorLease>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("lease protocol event cursor", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), &consumer)?;
            if stored
                .lease
                .as_ref()
                .is_some_and(|active| active.leased_until() > now)
            {
                return Ok(None);
            }
            let lease = ProtocolEventCursorLease::new(
                consumer.clone(),
                lease_id,
                stored.acknowledged_through,
                stored.attempt,
                now + Duration::from_millis(duration.get()),
            );
            stored.lease = Some(lease.clone());
            write_cursor(tx.as_mut(), &consumer, &stored)?;
            tx.commit()?;
            Ok(Some(lease))
        })
        .await
    }

    async fn acknowledge(
        &self,
        consumer: &ProtocolEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let consumer = consumer.clone();
        self.blocking("acknowledge protocol event cursor", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), &consumer)?;
            if stored
                .acknowledged_through
                .is_some_and(|current| through <= current)
            {
                return Ok(());
            }
            if stored.lease.is_some() {
                return Err(DomainError::Conflict {
                    what: "protocol_event_cursor",
                });
            }
            stored.acknowledged_through = Some(through);
            stored.attempt = ProtocolEventCursorAttempt::NONE;
            write_cursor(tx.as_mut(), &consumer, &stored)?;
            tx.commit()
        })
        .await
    }

    async fn acknowledge_lease(
        &self,
        lease: &ProtocolEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("acknowledge leased protocol event", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            require_lease(&stored, &lease, through)?;
            stored.acknowledged_through = Some(through);
            stored.attempt = ProtocolEventCursorAttempt::NONE;
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn mark_failed(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("fail protocol event cursor delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            require_lease(&stored, &lease, position)?;
            stored.attempt = stored.attempt.next();
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn quarantine(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
        reason: ProtocolEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("quarantine protocol event", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            require_lease(&stored, &lease, position)?;
            let quarantined = QuarantinedProtocolEvent::new(
                lease.consumer().clone(),
                position,
                stored.attempt,
                reason,
                now,
            );
            tx.insert(
                Table::EventCursorQuarantine,
                Key::Bytes(&quarantine_key(lease.consumer(), position)),
                &encode(&quarantined, "encode quarantined protocol event")?,
            )?;
            stored.acknowledged_through = Some(position);
            stored.attempt = ProtocolEventCursorAttempt::NONE;
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn release(&self, lease: &ProtocolEventCursorLease) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("release protocol event cursor", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            let matches = stored
                .lease
                .as_ref()
                .is_some_and(|active| active.lease_id() == lease.lease_id());
            if !matches {
                return Err(DomainError::Conflict {
                    what: "protocol_event_cursor",
                });
            }
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn quarantined(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Vec<QuarantinedProtocolEvent>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("read quarantined protocol events", move |engine| {
            let tx = engine.begin_read()?;
            let (start, end) = quarantine_range(&consumer);
            tx.scan_bytes_range(Table::EventCursorQuarantine, &start, &end)?
                .into_iter()
                .map(|(_, bytes)| decode(&bytes, "decode quarantined protocol event"))
                .collect()
        })
        .await
    }
}
