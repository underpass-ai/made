use async_trait::async_trait;
use made_core::entities::{AuditRecord, CeremonyCommit, CommitOutcome};
use made_core::error::DomainError;
use made_core::ports::CeremonyUnitOfWorkPort;
use made_core::value_objects::{CeremonyId, CeremonyRevision};

use crate::engine::{Key, Table};
use crate::redb::keys::{scope_range, scoped};
use crate::redb::StoredCeremony;

use super::audit_journal::journal_of;
use super::stored_outbox_message::StoredOutboxMessage;
use super::{decode, encode, RedbCeremonyStore};

#[async_trait]
impl CeremonyUnitOfWorkPort for RedbCeremonyStore {
    /// State, journal and outbox are written in one write transaction:
    /// redb commits all three tables together or none of them.
    async fn commit(&self, commit: CeremonyCommit) -> Result<CommitOutcome, DomainError> {
        self.blocking("commit", move |engine| {
            let ceremony_id = commit.instance().id().clone();
            let (instance, expected, facts, messages) = commit.into_parts();

            let mut tx = engine.begin_write()?;
            let outcome = {
                let stored: Option<StoredCeremony> = tx
                    .get(Table::Ceremonies, Key::Str(ceremony_id.as_str()))?
                    .map(|value| decode(&value, "decode ceremony"))
                    .transpose()?;
                let stored_revision = stored.map(|stored| stored.revision);

                if expected.matches(stored_revision) {
                    let mut head = journal_of(tx.as_ref(), &ceremony_id)?.pop();
                    let mut sealed = Vec::with_capacity(facts.len());
                    for fact in facts {
                        let record = match &head {
                            Some(previous) => AuditRecord::following(fact, previous)?,
                            None => AuditRecord::first(fact)?,
                        };
                        tx.insert(
                            Table::Journal,
                            Key::Bytes(&scoped(&ceremony_id, record.sequence().value())),
                            &encode(&record, "encode audit record")?,
                        )?;
                        head = Some(record.clone());
                        sealed.push(record);
                    }

                    let (start, end) = scope_range(&ceremony_id);
                    let enqueued = tx.scan_bytes_range(Table::Outbox, &start, &end)?.len() as u64;
                    for (offset, message) in messages.into_iter().enumerate() {
                        tx.insert(
                            Table::Outbox,
                            Key::Bytes(&scoped(&ceremony_id, enqueued + offset as u64)),
                            &encode(
                                &StoredOutboxMessage::enqueued(message),
                                "encode outbox message",
                            )?,
                        )?;
                    }

                    let revision = expected.resulting_revision();
                    tx.insert(
                        Table::Ceremonies,
                        Key::Str(ceremony_id.as_str()),
                        &encode(
                            &StoredCeremony {
                                revision,
                                instance: instance.clone(),
                            },
                            "encode ceremony",
                        )?,
                    )?;

                    CommitOutcome::Committed {
                        revision,
                        records: sealed,
                    }
                } else {
                    // Dropping the transaction without committing is
                    // what makes a rejected commit leave nothing behind.
                    CommitOutcome::Conflict {
                        expected,
                        stored: stored_revision,
                    }
                }
            };

            if outcome.is_conflict() {
                return Ok(outcome);
            }
            tx.commit()?;
            Ok(outcome)
        })
        .await
    }

    async fn revision(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Option<CeremonyRevision>, DomainError> {
        let ceremony_id = ceremony_id.clone();
        self.blocking("revision", move |engine| {
            let tx = engine.begin_read()?;
            let stored: Option<StoredCeremony> = tx
                .get(Table::Ceremonies, Key::Str(ceremony_id.as_str()))?
                .map(|value| decode(&value, "decode ceremony"))
                .transpose()?;
            Ok(stored.map(|stored| stored.revision))
        })
        .await
    }
}
