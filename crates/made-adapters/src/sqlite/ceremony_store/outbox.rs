use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::OutboxPort;
use made_core::value_objects::{ClaimedOutboxMessage, DurationMs, EventId, OutboxQuarantineReason};
use time::OffsetDateTime;

use crate::engine::{Key, ReadTx, Table};
use crate::sqlite::keys::ceremony_of;

use super::stored_outbox_message::StoredOutboxMessage;
use super::{decode, encode, SqliteCeremonyStore};

#[async_trait]
impl OutboxPort for SqliteCeremonyStore {
    /// Keys are grouped by ceremony and ordered within it, so one pass
    /// in key order visits each ceremony's queue in the order it was
    /// written. The first undelivered entry of a ceremony is its head,
    /// and it is the only one this claim can take: handing out two
    /// would put that ceremony's ordering in the publisher's hands.
    async fn claim(
        &self,
        limit: usize,
        now: OffsetDateTime,
        lease: DurationMs,
    ) -> Result<Vec<ClaimedOutboxMessage>, DomainError> {
        let lease_until = now + Duration::from_millis(lease.get());
        self.blocking("claim", move |engine| {
            let mut tx = engine.begin_write()?;
            let claimed = {
                let entries = read_outbox(tx.as_ref())?;

                let mut taken = Vec::new();
                let mut decided: Option<Vec<u8>> = None;
                for (key, stored) in entries {
                    let ceremony = ceremony_of(&key).unwrap_or_default().to_vec();
                    if decided.as_deref() == Some(ceremony.as_slice()) {
                        continue;
                    }
                    if stored.delivered {
                        continue;
                    }
                    // The head of this ceremony: claimable or not, it
                    // decides for the whole queue behind it.
                    decided = Some(ceremony);
                    if !stored.is_claimable(now) || taken.len() >= limit {
                        continue;
                    }
                    taken.push((key, stored));
                }

                let mut claimed = Vec::with_capacity(taken.len());
                for (key, mut stored) in taken {
                    stored.claimed_until = Some(lease_until);
                    tx.insert(
                        Table::Outbox,
                        Key::Bytes(&key),
                        &encode(&stored, "encode outbox message")?,
                    )?;
                    claimed.push(ClaimedOutboxMessage::new(stored.message, stored.attempt));
                }
                claimed
            };
            tx.commit()?;
            Ok(claimed)
        })
        .await
    }

    async fn mark_delivered(&self, event_ids: &[EventId]) -> Result<(), DomainError> {
        let event_ids = event_ids.to_vec();
        self.update_messages("mark_delivered", move |stored| {
            if event_ids.contains(stored.message.event_id()) {
                stored.delivered = true;
                stored.claimed_until = None;
                return true;
            }
            false
        })
        .await
    }

    async fn mark_failed(&self, event_id: &EventId) -> Result<(), DomainError> {
        let event_id = event_id.clone();
        self.update_messages("mark_failed", move |stored| {
            if stored.message.event_id() == &event_id {
                stored.attempt = stored.attempt.next();
                stored.claimed_until = None;
                return true;
            }
            false
        })
        .await
    }

    async fn quarantine(
        &self,
        event_id: &EventId,
        reason: OutboxQuarantineReason,
    ) -> Result<(), DomainError> {
        let event_id = event_id.clone();
        self.update_messages("quarantine", move |stored| {
            if stored.message.event_id() == &event_id {
                stored.quarantine = Some(reason.clone());
                stored.claimed_until = None;
                return true;
            }
            false
        })
        .await
    }

    async fn quarantined(&self) -> Result<Vec<ClaimedOutboxMessage>, DomainError> {
        self.blocking("quarantined", move |engine| {
            let tx = engine.begin_read()?;
            Ok(read_outbox(tx.as_ref())?
                .into_iter()
                .filter(|(_, stored)| stored.quarantine.is_some())
                .map(|(_, stored)| ClaimedOutboxMessage::new(stored.message, stored.attempt))
                .collect())
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// Apply `change` to every stored message it accepts, in one write
    /// transaction.
    async fn update_messages<F>(&self, op: &'static str, change: F) -> Result<(), DomainError>
    where
        F: Fn(&mut StoredOutboxMessage) -> bool + Send + 'static,
    {
        self.blocking(op, move |engine| {
            let mut tx = engine.begin_write()?;
            {
                let mut updates = Vec::new();
                for (key, mut stored) in read_outbox(tx.as_ref())? {
                    if change(&mut stored) {
                        updates.push((key, stored));
                    }
                }
                for (key, stored) in updates {
                    tx.insert(
                        Table::Outbox,
                        Key::Bytes(&key),
                        &encode(&stored, "encode outbox message")?,
                    )?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }
}

fn read_outbox(tx: &dyn ReadTx) -> Result<Vec<(Vec<u8>, StoredOutboxMessage)>, DomainError> {
    tx.scan_bytes(Table::Outbox)?
        .into_iter()
        .map(|(key, value)| Ok((key, decode(&value, "decode outbox message")?)))
        .collect()
}
