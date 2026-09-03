use async_trait::async_trait;
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::CeremonyInstanceRepositoryPort;
use made_core::value_objects::{CeremonyId, CeremonyRevision};

use crate::engine::{Key, Table};
use crate::redb::StoredCeremony;

use super::{decode, encode, RedbCeremonyStore};

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
