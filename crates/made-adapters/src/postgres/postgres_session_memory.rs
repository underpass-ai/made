use std::collections::{BTreeMap, BTreeSet, VecDeque};

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    MemoryReaderPort, MemoryRecollection, MemoryWriteOutcome, MemoryWriterPort,
};
use made_core::value_objects::{
    MemoryCapabilities, MemoryEntry, MemoryEntryId, MemoryMoment, MemoryRelation, MemoryScope,
    MemoryWrite,
};
use sqlx::Row;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::{PostgresCeremonyStore, PostgresPool};

/// Session memory persisted in the shared Postgres ceremony store.
#[derive(Debug, Clone)]
pub struct PostgresSessionMemory {
    pool: PostgresPool,
}

impl PostgresSessionMemory {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    #[must_use]
    fn declared() -> MemoryCapabilities {
        MemoryCapabilities::all()
    }

    async fn contents(
        &self,
        scope: &MemoryScope,
    ) -> Result<(Vec<MemoryEntry>, Vec<MemoryRelation>), DomainError> {
        let rows = sqlx::query(
            "SELECT payload FROM ceremony_memory_writes WHERE scope = $1 ORDER BY write_id",
        )
        .bind(scope.as_str())
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "recall ceremony memory"))?;
        let mut entries = Vec::new();
        let mut relations = Vec::new();
        for row in rows {
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode ceremony memory payload"))?;
            let (written_entries, written_relations): (Vec<MemoryEntry>, Vec<MemoryRelation>) =
                decode(&payload, "decode ceremony memory write")?;
            entries.extend(written_entries);
            relations.extend(written_relations);
        }
        Ok((entries, relations))
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

impl PostgresCeremonyStore {
    #[must_use]
    pub fn session_memory(&self) -> PostgresSessionMemory {
        PostgresSessionMemory::new(self.pool.clone())
    }
}

#[async_trait]
impl MemoryWriterPort for PostgresSessionMemory {
    async fn remember(
        &self,
        scope: &MemoryScope,
        write: MemoryWrite,
        idempotency_key: &str,
    ) -> Result<MemoryWriteOutcome, DomainError> {
        let payload = encode(
            &(write.entries(), write.relations()),
            "encode ceremony memory write",
        )?;
        let result = sqlx::query(
            "INSERT INTO ceremony_memory_writes (scope, idempotency_key, payload) \
             VALUES ($1, $2, $3) ON CONFLICT (scope, idempotency_key) DO NOTHING",
        )
        .bind(scope.as_str())
        .bind(idempotency_key)
        .bind(payload)
        .execute(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "remember ceremony memory"))?;
        if result.rows_affected() == 1 {
            Ok(MemoryWriteOutcome::Remembered)
        } else {
            Ok(MemoryWriteOutcome::AlreadyRemembered)
        }
    }

    fn capabilities(&self) -> MemoryCapabilities {
        Self::declared()
    }
}

#[async_trait]
impl MemoryReaderPort for PostgresSessionMemory {
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
        let entries = entries
            .into_iter()
            .filter(|entry| entry.provenance().observed_at() <= moment.instant())
            .collect::<Vec<_>>();
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
impl MemoryWriterPort for PostgresCeremonyStore {
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
        PostgresSessionMemory::declared()
    }
}

#[async_trait]
impl MemoryReaderPort for PostgresCeremonyStore {
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
        PostgresSessionMemory::declared()
    }
}
