use super::ceremony_store::{decode, encode};
use super::council_store::append;
use super::SqliteCouncilStore;
use crate::engine::{Key, ReadTx, Table};
use crate::stored_council_cursor::StoredCouncilCursor;
use async_trait::async_trait;
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord};
use made_core::error::DomainError;
use made_core::ports::CouncilJournalPort;
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalLeaseId, CouncilJournalPageLimit,
    CouncilJournalPosition, DurationMs,
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct SqliteCouncilJournal {
    store: SqliteCouncilStore,
}
impl SqliteCouncilJournal {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
}

fn cursor(
    tx: &dyn ReadTx,
    consumer: &CouncilJournalConsumer,
) -> Result<StoredCouncilCursor, DomainError> {
    tx.get(Table::CouncilJournalCursors, Key::Str(consumer.as_str()))?
        .map(|bytes| decode(&bytes, "load council cursor"))
        .transpose()
        .map(Option::unwrap_or_default)
}
fn require_lease(
    stored: &StoredCouncilCursor,
    lease: &CouncilJournalLease,
    now: OffsetDateTime,
) -> Result<(), DomainError> {
    if stored.lease.as_ref() != Some(lease) || lease.expires_at() <= now {
        return Err(DomainError::InvariantViolated {
            reason: "council consumer lease is expired or no longer owned",
        });
    }
    Ok(())
}

#[async_trait]
impl CouncilJournalPort for SqliteCouncilJournal {
    async fn publish(
        &self,
        event: CouncilJournalEvent,
    ) -> Result<CouncilJournalRecord, DomainError> {
        if event.publication_id().is_none() {
            return Err(DomainError::InvariantViolated {
                reason: "council publication requires an original event id",
            });
        }
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let record = append(tx.as_mut(), event)?;
                tx.commit()?;
                Ok(record)
            })
            .await
    }
    async fn read(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        self.store
            .blocking(move |engine| {
                let start = match after {
                    Some(after) => after.checked_next()?,
                    None => CouncilJournalPosition::FIRST,
                };
                let end = start.value().saturating_add(limit.value() as u64 - 1);
                engine
                    .begin_read()?
                    .scan_bytes_range(
                        Table::CouncilJournal,
                        &start.value().to_be_bytes(),
                        &end.to_be_bytes(),
                    )?
                    .into_iter()
                    .map(|(key, bytes)| {
                        let record: CouncilJournalRecord = decode(&bytes, "read council journal")?;
                        if key != record.position().value().to_be_bytes() {
                            return Err(DomainError::InvariantViolated {
                                reason: "council journal key differs from record position",
                            });
                        }
                        Ok(record)
                    })
                    .collect()
            })
            .await
    }
    async fn position(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        let consumer = consumer.clone();
        self.store
            .blocking(move |engine| Ok(cursor(engine.begin_read()?.as_ref(), &consumer)?.position))
            .await
    }
    async fn lease(
        &self,
        consumer: &CouncilJournalConsumer,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError> {
        if duration.get() == 0 || duration.get() > 3_600_000 {
            return Err(DomainError::InvariantViolated {
                reason: "council cursor lease must be between one millisecond and one hour",
            });
        }
        let expires_at = now
            .checked_add(time::Duration::milliseconds(duration.get() as i64))
            .ok_or(DomainError::InvariantViolated {
                reason: "council cursor lease deadline overflow",
            })?;
        let consumer = consumer.clone();
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let mut stored = cursor(tx.as_ref(), &consumer)?;
                if stored
                    .lease
                    .as_ref()
                    .is_some_and(|lease| lease.expires_at() > now)
                {
                    return Ok(None);
                }
                let lease = CouncilJournalLease::new(
                    consumer.clone(),
                    CouncilJournalLeaseId::new(uuid::Uuid::new_v4().to_string())?,
                    stored.position,
                    expires_at,
                );
                stored.lease = Some(lease.clone());
                tx.insert(
                    Table::CouncilJournalCursors,
                    Key::Str(consumer.as_str()),
                    &encode(&stored, "lease council cursor")?,
                )?;
                tx.commit()?;
                Ok(Some(lease))
            })
            .await
    }
    async fn acknowledge(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let mut stored = cursor(tx.as_ref(), lease.consumer())?;
                require_lease(&stored, &lease, now)?;
                if stored.position.is_some_and(|position| through < position)
                    || tx
                        .get(
                            Table::CouncilJournal,
                            Key::Bytes(&through.value().to_be_bytes()),
                        )?
                        .is_none()
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "council acknowledgement must advance to an existing record",
                    });
                }
                stored.position = Some(through);
                stored.lease = None;
                tx.insert(
                    Table::CouncilJournalCursors,
                    Key::Str(lease.consumer().as_str()),
                    &encode(&stored, "acknowledge council cursor")?,
                )?;
                tx.commit()
            })
            .await
    }
    async fn release(
        &self,
        lease: &CouncilJournalLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let mut stored = cursor(tx.as_ref(), lease.consumer())?;
                require_lease(&stored, &lease, now)?;
                stored.lease = None;
                tx.insert(
                    Table::CouncilJournalCursors,
                    Key::Str(lease.consumer().as_str()),
                    &encode(&stored, "release council cursor")?,
                )?;
                tx.commit()
            })
            .await
    }
}
