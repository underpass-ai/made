//! [`RedbCeremonyStore`] — the embedded durable store.
//!
//! Ceremony state, the audit journal and the outbox live in three
//! tables of one redb database, so a commit that touches all three is
//! one write transaction. Collaborating stores with a transaction each
//! would satisfy every property except the one that matters.
//!
//! redb is synchronous. Every operation runs on the blocking pool
//! rather than inline: a store call that blocks the async executor is
//! invisible until a host is under load, and then it is very visible.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use made_core::entities::{
    AuditFact, AuditRecord, CeremonyCommit, CeremonyInstance, CommitOutcome, PublicationOutcome,
    PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{
    AuditJournalPort, CeremonyDefinitionPublicationPort, CeremonyInstanceRepositoryPort,
    CeremonyUnitOfWorkPort, OutboxPort,
};
use made_core::value_objects::{
    CeremonyId, CeremonyName, CeremonyRevision, CeremonyVersion, ClaimedOutboxMessage, DurationMs,
    EventId, OutboxAttempt, OutboxMessage, OutboxQuarantineReason,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::engine::detect::{engine_of, StorageEngine};
use crate::engine::redb::RedbEngine;
use crate::engine::{Engine, Key, ReadTx, Table};

use super::error::{encoding_failure, join_failure};
use super::keys::{ceremony_of, published, scope_range, scoped};
use super::legacy_state_migration_receipt::LegacyStateMigrationReceipt;
use super::legacy_state_migrator::LegacyStateMigrator;
use super::{ConversionReceipt, StoredCeremony, StoredPublication};

/// A committed message and everything the store knows about getting it
/// out.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredOutboxMessage {
    message: OutboxMessage,
    attempt: OutboxAttempt,
    #[serde(default, with = "time::serde::rfc3339::option")]
    claimed_until: Option<OffsetDateTime>,
    delivered: bool,
    quarantine: Option<OutboxQuarantineReason>,
}

impl StoredOutboxMessage {
    fn enqueued(message: OutboxMessage) -> Self {
        Self {
            message,
            attempt: OutboxAttempt::NONE,
            claimed_until: None,
            delivered: false,
            quarantine: None,
        }
    }

    fn is_claimable(&self, now: OffsetDateTime) -> bool {
        !self.delivered
            && self.quarantine.is_none()
            && self.claimed_until.is_none_or(|until| until <= now)
    }
}

#[derive(Debug, Clone)]
pub struct RedbCeremonyStore {
    engine: Arc<dyn Engine>,
}

impl RedbCeremonyStore {
    /// Open, creating the database and its tables when absent.
    /// Open the store at `path`, on whichever engine wrote it.
    ///
    /// An existing store is opened by the engine its own bytes name, so it
    /// can never be opened by the wrong one. A path with no store yet gets
    /// redb, the default.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with(path, None)
    }

    /// Open on the WAL-mode SQLite engine, which several processes can hold
    /// at once. Only available when built with the `sqlite` feature.
    #[cfg(feature = "sqlite")]
    pub fn open_sqlite(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with(path, Some(StorageEngine::Sqlite))
    }

    /// [`open`](Self::open) with a say in the engine.
    ///
    /// `wanted` decides only what a **new** store becomes. An existing one
    /// always opens on the engine that wrote it, and asking for a different
    /// engine is refused by name rather than silently ignored — a store
    /// opened as the wrong engine is how a user ends up with two half-full
    /// files and no idea which is theirs.
    pub fn open_with(
        path: impl AsRef<Path>,
        wanted: Option<StorageEngine>,
    ) -> Result<Self, DomainError> {
        let path = path.as_ref();
        let engine = match (engine_of(path)?, wanted) {
            (Some(existing), Some(wanted)) if existing != wanted => {
                tracing::error!(
                    path = %path.display(),
                    existing = existing.name(),
                    requested = wanted.name(),
                    "refusing to open a ceremony store with an engine that did not write it"
                );
                return Err(DomainError::InvariantViolated {
                    reason: "the ceremony store was written by a different storage engine",
                });
            }
            (Some(existing), _) => existing,
            (None, wanted) => wanted.unwrap_or(StorageEngine::Redb),
        };
        Self::on(path, engine)
    }

    fn on(path: &Path, engine: StorageEngine) -> Result<Self, DomainError> {
        let engine: Arc<dyn Engine> = match engine {
            StorageEngine::Redb => Arc::new(RedbEngine::open(path)?),
            #[cfg(feature = "sqlite")]
            StorageEngine::Sqlite => Arc::new(crate::engine::sqlite::SqliteEngine::open(path)?),
            #[cfg(not(feature = "sqlite"))]
            StorageEngine::Sqlite => {
                tracing::error!(
                    path = %path.display(),
                    "the ceremony store is a SQLite database and this binary was built without that engine"
                );
                return Err(DomainError::InvariantViolated {
                    reason: "the ceremony store needs the sqlite engine, which this binary was \
                             built without; rebuild with --features sqlite",
                });
            }
        };
        Ok(Self { engine })
    }

    /// Import a pre-rename Choreographer redb file into a new MADE store.
    ///
    /// The source is opened with read-only file permissions. The destination
    /// must not exist, so migration cannot overwrite either the legacy
    /// evidence or an independently created MADE store.
    pub fn import_legacy(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<Self, DomainError> {
        let source = source.as_ref();
        let destination = destination.as_ref();

        if same_path(source, destination) {
            tracing::error!(
                migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
                "legacy state migration source and destination are the same file"
            );
            return Err(DomainError::InvariantViolated {
                reason: "legacy state migration requires different source and destination files",
            });
        }
        if destination.exists() {
            tracing::error!(
                migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
                "legacy state migration destination already exists"
            );
            return Err(DomainError::Conflict {
                what: "legacy_state_migration_destination",
            });
        }

        let receipt = LegacyStateMigrator::migrate(source, destination)?;
        tracing::info!(
            migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
            source_open_mode = "read_only",
            publications = receipt.publications(),
            migrated_publications = receipt.migrated_publications(),
            instances = receipt.instances(),
            migrated_instances = receipt.migrated_instances(),
            unresolved_bindings = receipt.unresolved_bindings(),
            audit_records = receipt.audit_records(),
            outbox_messages = receipt.outbox_messages(),
            source_sha256 = receipt.source_sha256(),
            "made legacy state migration completed"
        );
        Self::open(destination)
    }

    /// Import once, then reopen the already migrated destination on later
    /// process starts.
    pub fn open_or_import_legacy(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<Self, DomainError> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        if same_path(source, destination) {
            return Err(DomainError::InvariantViolated {
                reason: "legacy state migration requires different source and destination files",
            });
        }
        if !destination.exists() {
            return Self::import_legacy(source, destination);
        }

        let store = Self::open(destination)?;
        let receipt = store
            .legacy_migration_receipt()?
            .ok_or(DomainError::Conflict {
                what: "legacy_state_migration_destination_without_receipt",
            })?;
        tracing::info!(
            migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
            outcome = "already_completed",
            source_open_mode = "not_reopened",
            publications = receipt.publications(),
            migrated_publications = receipt.migrated_publications(),
            instances = receipt.instances(),
            migrated_instances = receipt.migrated_instances(),
            unresolved_bindings = receipt.unresolved_bindings(),
            source_sha256 = receipt.source_sha256(),
            "made legacy state migration receipt verified"
        );
        Ok(store)
    }

    /// The durable receipt written by a completed legacy import, if this
    /// store was created through [`Self::import_legacy`].
    pub fn legacy_migration_receipt(
        &self,
    ) -> Result<Option<LegacyStateMigrationReceipt>, DomainError> {
        let tx = self.engine.begin_read()?;
        tx.get(
            Table::LegacyStateMigrations,
            Key::Str(LegacyStateMigrationReceipt::MIGRATION_ID),
        )?
        .map(|value| decode(&value, "decode legacy migration receipt"))
        .transpose()
    }

    async fn blocking<T, F>(&self, op: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, op))?
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn encode<T: Serialize>(value: &T, op: &'static str) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(value).map_err(|error| encoding_failure(&error, op))
}

pub(super) fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    op: &'static str,
) -> Result<T, DomainError> {
    serde_json::from_slice(bytes).map_err(|error| encoding_failure(&error, op))
}

fn journal_of(tx: &dyn ReadTx, ceremony_id: &CeremonyId) -> Result<Vec<AuditRecord>, DomainError> {
    let (start, end) = scope_range(ceremony_id);
    tx.scan_bytes_range(Table::Journal, &start, &end)?
        .into_iter()
        .map(|(_, value)| decode(&value, "decode audit record"))
        .collect()
}

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

#[async_trait]
impl AuditJournalPort for RedbCeremonyStore {
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

#[async_trait]
impl OutboxPort for RedbCeremonyStore {
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

impl RedbCeremonyStore {
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

#[async_trait]
impl CeremonyDefinitionPublicationPort for RedbCeremonyStore {
    /// The occupant is read and the slot written inside one write
    /// transaction, so two callers cannot publish different content
    /// under one version.
    async fn publish(
        &self,
        definition: PublishedCeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        self.blocking("publish", move |engine| {
            let key = published(definition.name(), definition.version());
            let mut tx = engine.begin_write()?;
            let outcome = {
                let occupant: Option<StoredPublication> = tx
                    .get(Table::Publications, Key::Bytes(&key))?
                    .map(|value| decode(&value, "decode publication"))
                    .transpose()?;

                match occupant {
                    Some(occupant) if occupant.digest == definition.digest() => {
                        PublicationOutcome::AlreadyPublished(occupant.restore()?)
                    }
                    Some(occupant) => PublicationOutcome::VersionOccupied {
                        published: occupant.digest,
                        offered: definition.digest(),
                    },
                    None => {
                        tx.insert(
                            Table::Publications,
                            Key::Bytes(&key),
                            &encode(&StoredPublication::seal(&definition), "encode publication")?,
                        )?;
                        PublicationOutcome::Published(definition)
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

    async fn published(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        let key = published(name, version);
        self.blocking("published", move |engine| {
            let tx = engine.begin_read()?;
            let stored: Option<StoredPublication> = tx
                .get(Table::Publications, Key::Bytes(&key))?
                .map(|value| decode(&value, "decode publication"))
                .transpose()?;
            stored.map(StoredPublication::restore).transpose()
        })
        .await
    }

    async fn catalogue(&self) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        self.blocking("catalogue", move |engine| {
            let tx = engine.begin_read()?;
            tx.scan_bytes(Table::Publications)?
                .into_iter()
                .map(|(_, value)| {
                    decode::<StoredPublication>(&value, "decode publication")?.restore()
                })
                .collect()
        })
        .await
    }
}

#[async_trait]
impl CeremonyInstanceRepositoryPort for RedbCeremonyStore {
    /// Store an instance outside a unit of work.
    ///
    /// This is the path every ceremony use case takes today, and it
    /// carries no optimistic concurrency: it cannot, because the port
    /// has nowhere to put an expected revision. Making it durable is
    /// still strictly better than holding it in memory, and the
    /// transactional path stays available for callers that need both.
    ///
    /// The revision advances on every save even though nothing checks
    /// it here. That is deliberate: a concurrent
    /// [`CeremonyUnitOfWorkPort::commit`] holding a stale expectation
    /// then conflicts as it should, so the weaker path cannot quietly
    /// defeat the stronger one.
    async fn save(&self, instance: &CeremonyInstance) -> Result<(), DomainError> {
        let instance = instance.clone();
        self.blocking("save instance", move |engine| {
            let key = instance.id().as_str().to_owned();
            let mut tx = engine.begin_write()?;
            {
                let stored: Option<StoredCeremony> = tx
                    .get(Table::Ceremonies, Key::Str(&key))?
                    .map(|value| decode(&value, "decode ceremony"))
                    .transpose()?;
                let revision =
                    stored.map_or(CeremonyRevision::INITIAL, |stored| stored.revision.next());
                tx.insert(
                    Table::Ceremonies,
                    Key::Str(&key),
                    &encode(&StoredCeremony { revision, instance }, "encode ceremony")?,
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn get(&self, id: &CeremonyId) -> Result<CeremonyInstance, DomainError> {
        self.stored_instance(id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance",
            })
    }

    async fn list(&self) -> Result<Vec<CeremonyInstance>, DomainError> {
        self.blocking("list instances", move |engine| {
            let tx = engine.begin_read()?;
            tx.scan_str(Table::Ceremonies)?
                .into_iter()
                .map(|(_, value)| {
                    decode::<StoredCeremony>(&value, "decode ceremony").map(|s| s.instance)
                })
                .collect()
        })
        .await
    }

    async fn exists(&self, id: &CeremonyId) -> Result<bool, DomainError> {
        Ok(self.stored_instance(id).await?.is_some())
    }
}

impl RedbCeremonyStore {
    async fn stored_instance(
        &self,
        id: &CeremonyId,
    ) -> Result<Option<CeremonyInstance>, DomainError> {
        let key = id.as_str().to_owned();
        self.blocking("read instance", move |engine| {
            let tx = engine.begin_read()?;
            let stored: Option<StoredCeremony> = tx
                .get(Table::Ceremonies, Key::Str(&key))?
                .map(|value| decode(&value, "decode ceremony"))
                .transpose()?;
            Ok(stored.map(|stored| stored.instance))
        })
        .await
    }
}

impl RedbCeremonyStore {
    /// Copy a store to `destination` on `engine`, table for table.
    ///
    /// This is how a store changes engines. It is a copy rather than a replay
    /// of history, and that is not a shortcut: a ceremony store is state plus
    /// an audit journal, not a log with derived projections. Replaying the
    /// journal would rebuild the facts and lose the very thing the journal is
    /// evidence *of* — the instances, the outbox and the publications are
    /// authoritative, not derivable.
    ///
    /// What makes the copy safe is that both engines answer the same seam, so
    /// "every row of every table, in key order" means the same thing on each
    /// side. Rows move as opaque bytes: nothing is decoded, so nothing can be
    /// re-encoded differently.
    ///
    /// The source is only read. The destination must not already hold a
    /// store — converting into occupied memory would be a worse failure than
    /// the one it fixes.
    pub fn convert(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        engine: StorageEngine,
    ) -> Result<ConversionReceipt, DomainError> {
        let source_path = source.as_ref();
        let destination_path = destination.as_ref();

        if same_path(source_path, destination_path) {
            tracing::error!("ceremony store conversion source and destination are the same file");
            return Err(DomainError::InvariantViolated {
                reason: "store conversion requires different source and destination files",
            });
        }
        let Some(source_engine) = engine_of(source_path)? else {
            tracing::error!(path = %source_path.display(), "no ceremony store to convert");
            return Err(DomainError::NotFound {
                what: "ceremony_store",
            });
        };
        if source_engine == engine {
            tracing::error!(
                engine = engine.name(),
                "the ceremony store already runs on the requested engine"
            );
            return Err(DomainError::Conflict {
                what: "the store already runs on that engine",
            });
        }
        if engine_of(destination_path)?.is_some() {
            tracing::error!(
                path = %destination_path.display(),
                "refusing to convert into a path that already holds a ceremony store"
            );
            return Err(DomainError::AlreadyExists {
                what: "ceremony_store",
            });
        }

        let from = Self::on(source_path, source_engine)?;
        let into = Self::on(destination_path, engine)?;

        let read = from.engine.begin_read()?;
        let mut write = into.engine.begin_write()?;
        let mut receipt = ConversionReceipt {
            source_engine,
            destination_engine: engine,
            ceremonies: 0,
            journal_records: 0,
            outbox_messages: 0,
            publications: 0,
        };

        for (table, counter) in [
            (Table::Ceremonies, &mut receipt.ceremonies),
            (Table::LegacyStateMigrations, &mut 0),
        ] {
            for (key, value) in read.scan_str(table)? {
                write.insert(table, Key::Str(&key), &value)?;
                *counter += 1;
            }
        }
        for (table, counter) in [
            (Table::Journal, &mut receipt.journal_records),
            (Table::Outbox, &mut receipt.outbox_messages),
            (Table::Publications, &mut receipt.publications),
        ] {
            for (key, value) in read.scan_bytes(table)? {
                write.insert(table, Key::Bytes(&key), &value)?;
                *counter += 1;
            }
        }

        write.commit()?;
        Ok(receipt)
    }
}
