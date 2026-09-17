//! Durable session memory in the canonical embedded SQLite store.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    MemoryReaderPort, MemoryRecollection, MemoryWriteOutcome, MemoryWriterPort,
};
use made_core::value_objects::{
    MemoryCapabilities, MemoryEntry, MemoryEntryId, MemoryMoment, MemoryRelation, MemoryScope,
    MemoryWrite,
};

use crate::engine::{Engine, Key, Table};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::keys::{memory_scope_range, memory_write};
use super::SqliteCeremonyStore;

/// Session memory persisted beside ceremony streams in one SQLite engine.
#[derive(Debug, Clone)]
pub struct SqliteSessionMemory {
    engine: Arc<dyn Engine>,
}

impl SqliteSessionMemory {
    /// Open durable memory at `path` through the canonical ceremony-store
    /// lifecycle checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.session_memory())
    }

    pub(super) fn from_engine(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }

    #[must_use]
    fn declared() -> MemoryCapabilities {
        MemoryCapabilities::all()
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

    async fn contents(
        &self,
        scope: &MemoryScope,
    ) -> Result<(Vec<MemoryEntry>, Vec<MemoryRelation>), DomainError> {
        let (start, end) = memory_scope_range(scope);
        self.blocking("recall memory", move |engine| {
            let tx = engine.begin_read()?;
            let mut entries = Vec::new();
            let mut relations = Vec::new();
            for (_, bytes) in tx.scan_bytes_range(Table::MemoryWrites, &start, &end)? {
                let (written_entries, written_relations): (Vec<MemoryEntry>, Vec<MemoryRelation>) =
                    decode(&bytes, "decode memory write")?;
                entries.extend(written_entries);
                relations.extend(written_relations);
            }
            Ok((entries, relations))
        })
        .await
    }

    fn between(relations: &[MemoryRelation], visible: &[MemoryEntry]) -> Vec<MemoryRelation> {
        let ids: BTreeSet<&MemoryEntryId> = visible.iter().map(MemoryEntry::id).collect();
        relations
            .iter()
            .filter(|relation| ids.contains(relation.from()) && ids.contains(relation.to()))
            .cloned()
            .collect()
    }

    fn chain(
        relations: &[MemoryRelation],
        from: &MemoryEntryId,
        to: &MemoryEntryId,
    ) -> Vec<MemoryRelation> {
        let mut frontier = VecDeque::from([from.clone()]);
        let mut arrived_by: BTreeMap<MemoryEntryId, MemoryRelation> = BTreeMap::new();
        let mut seen = BTreeSet::from([from.clone()]);

        while let Some(here) = frontier.pop_front() {
            if &here == to {
                break;
            }
            for relation in relations.iter().filter(|relation| relation.from() == &here) {
                if seen.insert(relation.to().clone()) {
                    arrived_by.insert(relation.to().clone(), relation.clone());
                    frontier.push_back(relation.to().clone());
                }
            }
        }

        let mut chain = Vec::new();
        let mut here = to.clone();
        while let Some(relation) = arrived_by.get(&here) {
            chain.push(relation.clone());
            here = relation.from().clone();
        }
        chain.reverse();
        chain
    }
}

impl SqliteCeremonyStore {
    /// Memory sharing this store's already-open engine and transaction pool.
    #[must_use]
    pub fn session_memory(&self) -> SqliteSessionMemory {
        SqliteSessionMemory::from_engine(Arc::clone(&self.engine))
    }
}

#[async_trait]
impl MemoryWriterPort for SqliteSessionMemory {
    async fn remember(
        &self,
        scope: &MemoryScope,
        write: MemoryWrite,
        idempotency_key: &str,
    ) -> Result<MemoryWriteOutcome, DomainError> {
        let key = memory_write(scope, idempotency_key);
        let value = encode(&(write.entries(), write.relations()), "encode memory write")?;
        self.blocking("remember memory", move |engine| {
            let mut tx = engine.begin_write()?;
            if tx.get(Table::MemoryWrites, Key::Bytes(&key))?.is_some() {
                return Ok(MemoryWriteOutcome::AlreadyRemembered);
            }
            tx.insert(Table::MemoryWrites, Key::Bytes(&key), &value)?;
            tx.commit()?;
            Ok(MemoryWriteOutcome::Remembered)
        })
        .await
    }

    fn capabilities(&self) -> MemoryCapabilities {
        Self::declared()
    }
}

#[async_trait]
impl MemoryReaderPort for SqliteSessionMemory {
    async fn recall(&self, scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
        let (entries, relations) = self.contents(scope).await?;
        if entries.is_empty() && relations.is_empty() {
            Ok(MemoryRecollection::nothing())
        } else {
            Ok(MemoryRecollection::Recalled { entries, relations })
        }
    }

    async fn as_known_at(
        &self,
        scope: &MemoryScope,
        moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError> {
        let (entries, relations) = self.contents(scope).await?;
        let entries: Vec<_> = entries
            .into_iter()
            .filter(|entry| entry.provenance().observed_at() <= moment.instant())
            .collect();
        let relations = Self::between(&relations, &entries);
        if entries.is_empty() && relations.is_empty() {
            Ok(MemoryRecollection::nothing())
        } else {
            Ok(MemoryRecollection::Recalled { entries, relations })
        }
    }

    async fn follow(
        &self,
        scope: &MemoryScope,
        from: &MemoryEntryId,
        to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError> {
        let (_, relations) = self.contents(scope).await?;
        let relations = Self::chain(&relations, from, to);
        if relations.is_empty() {
            Ok(MemoryRecollection::nothing())
        } else {
            Ok(MemoryRecollection::Recalled {
                entries: Vec::new(),
                relations,
            })
        }
    }

    fn capabilities(&self) -> MemoryCapabilities {
        Self::declared()
    }
}

#[async_trait]
impl MemoryWriterPort for SqliteCeremonyStore {
    async fn remember(
        &self,
        scope: &MemoryScope,
        write: MemoryWrite,
        idempotency_key: &str,
    ) -> Result<MemoryWriteOutcome, DomainError> {
        self.session_memory()
            .remember(scope, write, idempotency_key)
            .await
    }

    fn capabilities(&self) -> MemoryCapabilities {
        SqliteSessionMemory::declared()
    }
}

#[async_trait]
impl MemoryReaderPort for SqliteCeremonyStore {
    async fn recall(&self, scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
        self.session_memory().recall(scope).await
    }

    async fn as_known_at(
        &self,
        scope: &MemoryScope,
        moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError> {
        self.session_memory().as_known_at(scope, moment).await
    }

    async fn follow(
        &self,
        scope: &MemoryScope,
        from: &MemoryEntryId,
        to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError> {
        self.session_memory().follow(scope, from, to).await
    }

    fn capabilities(&self) -> MemoryCapabilities {
        SqliteSessionMemory::declared()
    }
}
