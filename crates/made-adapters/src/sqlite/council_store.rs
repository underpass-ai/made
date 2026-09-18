use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::SqliteCeremonyStore;
use crate::engine::{Engine, Key, Table, WriteTx};
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord};
use made_core::error::DomainError;
use made_core::value_objects::CouncilJournalPosition;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

/// Shared transactional storage for council adapters, in the same database as
/// ceremonies but with an independent journal and cursor namespace.
#[derive(Debug, Clone)]
pub struct SqliteCouncilStore {
    engine: Arc<dyn Engine>,
}

impl SqliteCouncilStore {
    #[must_use]
    pub fn over(store: &SqliteCeremonyStore) -> Self {
        Self {
            engine: store.engine.clone(),
        }
    }
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, DomainError> {
        Ok(Self::over(&SqliteCeremonyStore::open(path)?))
    }
    pub(super) async fn blocking<T, F>(&self, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, "council store"))?
    }
    pub(super) async fn get<T: DeserializeOwned + Send + 'static>(
        &self,
        table: Table,
        key: String,
        what: &'static str,
    ) -> Result<T, DomainError> {
        self.blocking(move |engine| {
            let tx = engine.begin_read()?;
            let bytes = tx
                .get(table, Key::Str(&key))?
                .ok_or(DomainError::NotFound { what })?;
            decode(&bytes, "read council value")
        })
        .await
    }
    pub(super) async fn contains(&self, table: Table, key: String) -> Result<bool, DomainError> {
        self.blocking(move |engine| Ok(engine.begin_read()?.get(table, Key::Str(&key))?.is_some()))
            .await
    }
    pub(super) async fn list<T: DeserializeOwned + Send + 'static>(
        &self,
        table: Table,
    ) -> Result<Vec<T>, DomainError> {
        self.blocking(move |engine| {
            engine
                .begin_read()?
                .scan_str(table)?
                .into_iter()
                .map(|(_, bytes)| decode(&bytes, "list council values"))
                .collect()
        })
        .await
    }
    pub(super) async fn insert<T: Serialize + Send + 'static>(
        &self,
        table: Table,
        key: String,
        value: T,
        what: &'static str,
        event: CouncilJournalEvent,
    ) -> Result<(), DomainError> {
        self.blocking(move |engine| {
            let mut tx = engine.begin_write()?;
            if tx.get(table, Key::Str(&key))?.is_some() {
                return Err(DomainError::AlreadyExists { what });
            }
            tx.insert(
                table,
                Key::Str(&key),
                &encode(&value, "insert council value")?,
            )?;
            append(tx.as_mut(), event)?;
            tx.commit()
        })
        .await
    }
    pub(super) async fn delete(
        &self,
        table: Table,
        key: String,
        what: &'static str,
        event: CouncilJournalEvent,
    ) -> Result<(), DomainError> {
        self.blocking(move |engine| {
            let mut tx = engine.begin_write()?;
            if tx.get(table, Key::Str(&key))?.is_none() {
                return Err(DomainError::NotFound { what });
            }
            tx.remove(table, Key::Str(&key))?;
            append(tx.as_mut(), event)?;
            tx.commit()
        })
        .await
    }
}

pub(super) fn append(
    tx: &mut dyn WriteTx,
    event: CouncilJournalEvent,
) -> Result<CouncilJournalRecord, DomainError> {
    if let Some(id) = event.publication_id() {
        if let Some(bytes) = tx.get(Table::CouncilJournalIds, Key::Str(id.as_str()))? {
            let record: CouncilJournalRecord = decode(&bytes, "read council publication")?;
            if record.event() != &event {
                return Err(DomainError::InvariantViolated {
                    reason: "council publication id already has a different fact",
                });
            }
            return Ok(record);
        }
    }
    let position = match tx.get(Table::Meta, Key::Str("council_journal_last_position"))? {
        Some(bytes) => {
            decode::<CouncilJournalPosition>(&bytes, "read council ordinal")?.checked_next()?
        }
        None => CouncilJournalPosition::FIRST,
    };
    let record = CouncilJournalRecord::new(position, event);
    let bytes = encode(&record, "write council record")?;
    tx.insert(
        Table::CouncilJournal,
        Key::Bytes(&position.value().to_be_bytes()),
        &bytes,
    )?;
    if let Some(id) = record.event().publication_id() {
        tx.insert(Table::CouncilJournalIds, Key::Str(id.as_str()), &bytes)?;
    }
    tx.insert(
        Table::Meta,
        Key::Str("council_journal_last_position"),
        &encode(&position, "write council ordinal")?,
    )?;
    Ok(record)
}
