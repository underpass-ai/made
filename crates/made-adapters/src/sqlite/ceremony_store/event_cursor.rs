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

use crate::engine::{Key, ReadTx, Table, WriteTx};

use super::stored_cursor::StoredCursor;
use super::{decode, encode, SqliteCeremonyStore};

fn read_cursor(
    tx: &dyn ReadTx,
    consumer: &CeremonyEventConsumer,
) -> Result<StoredCursor, DomainError> {
    tx.get(Table::EventCursors, Key::Str(consumer.as_str()))?
        .map(|bytes| decode(&bytes, "decode ceremony event cursor"))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn write_cursor(
    tx: &mut dyn WriteTx,
    consumer: &CeremonyEventConsumer,
    cursor: &StoredCursor,
) -> Result<(), DomainError> {
    tx.insert(
        Table::EventCursors,
        Key::Str(consumer.as_str()),
        &encode(cursor, "encode ceremony event cursor")?,
    )
}

fn quarantine_key(consumer: &CeremonyEventConsumer, position: GlobalPosition) -> Vec<u8> {
    let mut key = consumer.as_str().as_bytes().to_vec();
    key.push(0);
    key.extend_from_slice(&position.value().to_be_bytes());
    key
}

fn quarantine_range(consumer: &CeremonyEventConsumer) -> (Vec<u8>, Vec<u8>) {
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
    lease: &CeremonyEventCursorLease,
    position: GlobalPosition,
) -> Result<(), DomainError> {
    let matches = stored
        .lease
        .as_ref()
        .is_some_and(|active| active.lease_id() == lease.lease_id());
    if !matches || position != lease.next_position() {
        return Err(DomainError::Conflict {
            what: "ceremony_event_cursor",
        });
    }
    Ok(())
}

#[async_trait]
impl CeremonyEventCursorPort for SqliteCeremonyStore {
    async fn position(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("read ceremony event cursor", move |engine| {
            let tx = engine.begin_read()?;
            Ok(read_cursor(tx.as_ref(), &consumer)?.acknowledged_through)
        })
        .await
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("lease ceremony event cursor", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), &consumer)?;
            if stored
                .lease
                .as_ref()
                .is_some_and(|active| active.leased_until() > now)
            {
                return Ok(None);
            }
            let lease = CeremonyEventCursorLease::new(
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
        consumer: &CeremonyEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let consumer = consumer.clone();
        self.blocking("acknowledge ceremony event cursor", move |engine| {
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
                    what: "ceremony_event_cursor",
                });
            }
            stored.acknowledged_through = Some(through);
            stored.attempt = CeremonyEventCursorAttempt::NONE;
            write_cursor(tx.as_mut(), &consumer, &stored)?;
            tx.commit()
        })
        .await
    }

    async fn acknowledge_lease(
        &self,
        lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("acknowledge leased ceremony event", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            require_lease(&stored, &lease, through)?;
            stored.acknowledged_through = Some(through);
            stored.attempt = CeremonyEventCursorAttempt::NONE;
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn mark_failed(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("fail ceremony event cursor delivery", move |engine| {
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
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        reason: CeremonyEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("quarantine ceremony event", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            require_lease(&stored, &lease, position)?;
            let quarantined = QuarantinedCeremonyEvent::new(
                lease.consumer().clone(),
                position,
                stored.attempt,
                reason,
                now,
            );
            tx.insert(
                Table::EventCursorQuarantine,
                Key::Bytes(&quarantine_key(lease.consumer(), position)),
                &encode(&quarantined, "encode quarantined ceremony event")?,
            )?;
            stored.acknowledged_through = Some(position);
            stored.attempt = CeremonyEventCursorAttempt::NONE;
            stored.lease = None;
            write_cursor(tx.as_mut(), lease.consumer(), &stored)?;
            tx.commit()
        })
        .await
    }

    async fn release(&self, lease: &CeremonyEventCursorLease) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("release ceremony event cursor", move |engine| {
            let mut tx = engine.begin_write()?;
            let mut stored = read_cursor(tx.as_ref(), lease.consumer())?;
            let matches = stored
                .lease
                .as_ref()
                .is_some_and(|active| active.lease_id() == lease.lease_id());
            if !matches {
                return Err(DomainError::Conflict {
                    what: "ceremony_event_cursor",
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
        consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError> {
        let consumer = consumer.clone();
        self.blocking("read quarantined ceremony events", move |engine| {
            let tx = engine.begin_read()?;
            let (start, end) = quarantine_range(&consumer);
            tx.scan_bytes_range(Table::EventCursorQuarantine, &start, &end)?
                .into_iter()
                .map(|(_, bytes)| decode(&bytes, "decode quarantined ceremony event"))
                .collect()
        })
        .await
    }
}
