use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord};
use made_core::error::DomainError;
use made_core::ports::AuditJournalPort;
use made_core::value_objects::CeremonyId;

use crate::engine::{Key, ReadTx, Table};
use crate::sqlite::keys::{scope_range, scoped};

use super::{decode, encode, SqliteCeremonyStore};

pub(super) fn journal_of(
    tx: &dyn ReadTx,
    ceremony_id: &CeremonyId,
) -> Result<Vec<AuditRecord>, DomainError> {
    let (start, end) = scope_range(ceremony_id);
    tx.scan_bytes_range(Table::Journal, &start, &end)?
        .into_iter()
        .map(|(_, value)| decode(&value, "decode audit record"))
        .collect()
}

#[async_trait]
impl AuditJournalPort for SqliteCeremonyStore {
    async fn append(&self, fact: AuditFact) -> Result<AuditRecord, DomainError> {
        self.blocking("append", move |engine| {
            let ceremony_id = fact.ceremony_id.clone();
            let mut tx = engine.begin_write()?;
            let record = {
                let head = journal_of(tx.as_ref(), &ceremony_id)?.pop();
                let record = match head {
                    Some(previous) => AuditRecord::following(fact, &previous)?,
                    None => AuditRecord::first(fact)?,
                };
                tx.insert(
                    Table::Journal,
                    Key::Bytes(&scoped(&ceremony_id, record.sequence().value())),
                    &encode(&record, "encode audit record")?,
                )?;
                record
            };
            tx.commit()?;
            Ok(record)
        })
        .await
    }

    async fn head(&self, ceremony_id: &CeremonyId) -> Result<Option<AuditRecord>, DomainError> {
        Ok(self.records(ceremony_id).await?.pop())
    }

    async fn records(&self, ceremony_id: &CeremonyId) -> Result<Vec<AuditRecord>, DomainError> {
        let ceremony_id = ceremony_id.clone();
        self.blocking("records", move |engine| {
            let tx = engine.begin_read()?;
            journal_of(tx.as_ref(), &ceremony_id)
        })
        .await
    }
}
