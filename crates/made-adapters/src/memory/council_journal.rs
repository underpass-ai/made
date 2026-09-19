use super::council_journal_state::CouncilJournalState;
use async_trait::async_trait;
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord};
use made_core::error::DomainError;
use made_core::ports::CouncilJournalPort;
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalLeaseId, CouncilJournalPageLimit,
    CouncilJournalPosition, DurationMs,
};
use std::sync::{Arc, Mutex, MutexGuard};
use time::OffsetDateTime;

/// An explicitly ephemeral journal for custom in-process hosts.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCouncilJournal {
    state: Arc<Mutex<CouncilJournalState>>,
}
impl InMemoryCouncilJournal {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> Result<MutexGuard<'_, CouncilJournalState>, DomainError> {
        self.state
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "in-memory council journal is poisoned",
            })
    }
}
#[async_trait]
impl CouncilJournalPort for InMemoryCouncilJournal {
    async fn publish(
        &self,
        event: CouncilJournalEvent,
    ) -> Result<CouncilJournalRecord, DomainError> {
        let id = event
            .publication_id()
            .ok_or(DomainError::InvariantViolated {
                reason: "council publication requires an original event id",
            })?
            .as_str()
            .to_owned();
        let mut state = self.lock()?;
        if let Some(existing) = state.publications.get(&id) {
            if existing.event() != &event {
                return Err(DomainError::InvariantViolated {
                    reason: "council publication id already has a different fact",
                });
            }
            return Ok(existing.clone());
        }
        let position = match state.records.last() {
            Some(record) => record.position().checked_next()?,
            None => CouncilJournalPosition::FIRST,
        };
        let record = match made_app::services::AuthorizationOperationScope::current() {
            Some(operation) => {
                CouncilJournalRecord::authorized(position, event, operation.evidence().clone())
            }
            None => CouncilJournalRecord::new(position, event),
        };
        state.records.push(record.clone());
        state.publications.insert(id, record.clone());
        Ok(record)
    }
    async fn read(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        Ok(self
            .lock()?
            .records
            .iter()
            .filter(|record| after.is_none_or(|after| record.position() > after))
            .take(limit.value())
            .cloned()
            .collect())
    }
    async fn position(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        Ok(self
            .lock()?
            .cursors
            .get(consumer.as_str())
            .and_then(|cursor| cursor.position))
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
        let mut state = self.lock()?;
        let stored = state
            .cursors
            .entry(consumer.as_str().to_owned())
            .or_default();
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
        Ok(Some(lease))
    }
    async fn acknowledge(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut state = self.lock()?;
        let exists = state
            .records
            .iter()
            .any(|record| record.position() == through);
        let stored =
            state
                .cursors
                .get_mut(lease.consumer().as_str())
                .ok_or(DomainError::NotFound {
                    what: "council_cursor",
                })?;
        if stored.lease.as_ref() != Some(lease) || lease.expires_at() <= now {
            return Err(DomainError::InvariantViolated {
                reason: "council consumer lease is expired or no longer owned",
            });
        }
        if !exists || stored.position.is_some_and(|position| through < position) {
            return Err(DomainError::InvariantViolated {
                reason: "council acknowledgement must advance to an existing record",
            });
        }
        stored.position = Some(through);
        stored.lease = None;
        Ok(())
    }
    async fn release(
        &self,
        lease: &CouncilJournalLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut state = self.lock()?;
        let stored =
            state
                .cursors
                .get_mut(lease.consumer().as_str())
                .ok_or(DomainError::NotFound {
                    what: "council_cursor",
                })?;
        if stored.lease.as_ref() != Some(lease) || lease.expires_at() <= now {
            return Err(DomainError::InvariantViolated {
                reason: "council consumer lease is expired or no longer owned",
            });
        }
        stored.lease = None;
        Ok(())
    }
}
