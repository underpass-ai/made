use super::ceremony_store::encode;
use super::council_store::append;
use super::SqliteCouncilStore;
use crate::engine::{Key, Table};
use async_trait::async_trait;
use made_core::entities::{CouncilJournalEvent, Deliberation};
use made_core::error::DomainError;
use made_core::ports::DeliberationRepositoryPort;
use made_core::value_objects::TaskId;

#[derive(Debug, Clone)]
pub struct SqliteDeliberationRepository {
    store: SqliteCouncilStore,
}
impl SqliteDeliberationRepository {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
}
#[async_trait]
impl DeliberationRepositoryPort for SqliteDeliberationRepository {
    async fn save(&self, deliberation: &Deliberation) -> Result<(), DomainError> {
        let deliberation = deliberation.clone();
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let key = Key::Str(deliberation.task_id().as_str());
                let bytes = encode(&deliberation, "save deliberation")?;
                if tx.get(Table::CouncilDeliberations, key)?.as_ref() == Some(&bytes) {
                    return Ok(());
                }
                tx.insert(Table::CouncilDeliberations, key, &bytes)?;
                append(
                    tx.as_mut(),
                    CouncilJournalEvent::DeliberationSnapshotSaved(deliberation),
                )?;
                tx.commit()
            })
            .await
    }
    async fn get(&self, id: &TaskId) -> Result<Deliberation, DomainError> {
        self.store
            .get(Table::CouncilDeliberations, id.to_string(), "deliberation")
            .await
    }
    async fn exists(&self, id: &TaskId) -> Result<bool, DomainError> {
        self.store
            .contains(Table::CouncilDeliberations, id.to_string())
            .await
    }
}
