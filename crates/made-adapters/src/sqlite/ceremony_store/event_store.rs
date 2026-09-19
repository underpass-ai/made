//! [`CeremonyEventStorePort`] over the storage seam.
//!
//! Three tables carry a stream: `ceremony_events` holds the sealed
//! records keyed by `(ceremony_id, sequence)`, `ceremony_event_log`
//! maps each global position to the events-table key of the record
//! filed there, and one row of `store_meta` remembers the last position
//! handed out. All three move in the one write transaction an append
//! opens, and `BEGIN IMMEDIATE` serialises appends store-wide, which is
//! what lets positions be assigned by reading a counter and bumping it.

use std::collections::BTreeSet;

use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord};
use made_core::error::DomainError;
use made_core::ports::{
    seal_continuation, AppendOutcome, CeremonyEventStorePort, PositionedRecord,
};
use made_core::value_objects::{
    AuthorizationEvidence, CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion,
};

use crate::engine::{Key, ReadTx, Table};
use crate::sqlite::keys::{ceremony_of, position, scoped};

use super::stored_event::StoredEvent;
use super::{decode, encode, SqliteCeremonyStore};

/// The `store_meta` row holding the last global position assigned.
const LAST_POSITION: &str = "last_global_position";

impl SqliteCeremonyStore {
    async fn append_with_authorization(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<AppendOutcome, DomainError> {
        let stream = stream.clone();
        self.blocking("append events", move |engine| {
            let mut tx = engine.begin_write()?;
            let existing = records_after(tx.as_ref(), &stream, StreamVersion::EMPTY, None)?;
            let actual = version_of(&existing);
            if actual != expected {
                return Ok(AppendOutcome::Conflict { expected, actual });
            }
            let sealed = seal_continuation(&stream, &existing, facts, authorization.as_ref())?;
            let first_position =
                last_position(tx.as_ref())?.map_or(GlobalPosition::FIRST, GlobalPosition::next);
            let mut next = first_position;
            let mut last = first_position;
            for record in &sealed {
                let key = scoped(&stream, record.sequence().value());
                let stored = StoredEvent {
                    position: next,
                    record: record.clone(),
                };
                tx.insert(
                    Table::Events,
                    Key::Bytes(&key),
                    &encode(&stored, "encode stored event")?,
                )?;
                tx.insert(Table::EventLog, Key::Bytes(&position(next.value())), &key)?;
                last = next;
                next = next.next();
            }
            tx.insert(
                Table::Meta,
                Key::Str(LAST_POSITION),
                &encode(&last, "encode last global position")?,
            )?;
            tx.commit()?;
            Ok(AppendOutcome::Appended {
                version: sealed.last().map_or(actual, |record| {
                    StreamVersion::from_sequence(record.sequence())
                }),
                records: sealed,
                first_position,
            })
        })
        .await
    }
}

/// The records of `stream` with a sequence above `after`, in order.
fn records_after(
    tx: &dyn ReadTx,
    stream: &CeremonyId,
    after: StreamVersion,
    limit: Option<CeremonyEventPageLimit>,
) -> Result<Vec<AuditRecord>, DomainError> {
    let start = scoped(stream, after.value().saturating_add(1));
    let end = scoped(stream, u64::MAX);
    tx.scan_bytes_range(Table::Events, &start, &end)?
        .into_iter()
        .take(limit.map_or(usize::MAX, CeremonyEventPageLimit::value))
        .map(|(_, value)| decode::<StoredEvent>(&value, "decode stored event").map(|e| e.record))
        .collect()
}

fn version_of(records: &[AuditRecord]) -> StreamVersion {
    records.last().map_or(StreamVersion::EMPTY, |record| {
        StreamVersion::from_sequence(record.sequence())
    })
}

fn last_position(tx: &dyn ReadTx) -> Result<Option<GlobalPosition>, DomainError> {
    tx.get(Table::Meta, Key::Str(LAST_POSITION))?
        .map(|value| decode(&value, "decode last global position"))
        .transpose()
}

#[async_trait]
impl CeremonyEventStorePort for SqliteCeremonyStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.append_with_authorization(stream, expected, facts, None)
            .await
    }

    async fn append_authorized(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
        authorization: AuthorizationEvidence,
    ) -> Result<AppendOutcome, DomainError> {
        self.append_with_authorization(stream, expected, facts, Some(authorization))
            .await
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let stream = stream.clone();
        self.blocking("read events", move |engine| {
            let tx = engine.begin_read()?;
            records_after(tx.as_ref(), &stream, after, Some(limit))
        })
        .await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.blocking("read all events", move |engine| {
            let tx = engine.begin_read()?;
            // Positions are contiguous, so the log range that ends
            // `limit - 1` past `from` holds exactly the page wanted.
            let span = u64::try_from(limit.value())
                .unwrap_or(u64::MAX)
                .saturating_sub(1);
            let end = from.value().saturating_add(span);
            tx.scan_bytes_range(Table::EventLog, &position(from.value()), &position(end))?
                .into_iter()
                .take(limit.value())
                .map(|(_, events_key)| {
                    let value = tx.get(Table::Events, Key::Bytes(&events_key))?.ok_or(
                        DomainError::InvariantViolated {
                            reason: "sqlite: the event log points at a missing event",
                        },
                    )?;
                    let stored: StoredEvent = decode(&value, "decode stored event")?;
                    Ok(PositionedRecord {
                        position: stored.position,
                        record: stored.record,
                    })
                })
                .collect()
        })
        .await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        let stream = stream.clone();
        self.blocking("stream head", move |engine| {
            let tx = engine.begin_read()?;
            Ok(version_of(&records_after(
                tx.as_ref(),
                &stream,
                StreamVersion::EMPTY,
                None,
            )?))
        })
        .await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.blocking("list streams", move |engine| {
            let tx = engine.begin_read()?;
            let mut streams = BTreeSet::new();
            for (key, _) in tx.scan_bytes(Table::Events)? {
                let id = ceremony_of(&key).ok_or(DomainError::InvariantViolated {
                    reason: "sqlite: an events-table key is too short to hold a ceremony id",
                })?;
                streams.insert(CeremonyId::new(String::from_utf8_lossy(id))?);
            }
            Ok(streams.into_iter().collect())
        })
        .await
    }
}
