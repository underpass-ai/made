use super::ceremony_store::{decode, encode};
use super::council_store::append;
use super::SqliteCouncilStore;
use crate::engine::{Key, Table};
use async_trait::async_trait;
use made_core::entities::{CouncilJournalEvent, Statistics};
use made_core::error::DomainError;
use made_core::ports::StatisticsPort;
use made_core::value_objects::{DurationMs, Specialty};

#[derive(Debug, Clone)]
pub struct SqliteCouncilStatistics {
    store: SqliteCouncilStore,
}
impl SqliteCouncilStatistics {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
    async fn record(
        &self,
        specialty: Option<Specialty>,
        duration: DurationMs,
    ) -> Result<(), DomainError> {
        let authorization = made_app::services::AuthorizationOperationScope::current()
            .map(|operation| operation.evidence().clone());
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let mut stats = tx
                    .get(Table::CouncilStatistics, Key::Str("totals"))?
                    .map(|bytes| decode::<Statistics>(&bytes, "load statistics"))
                    .transpose()?
                    .unwrap_or_default();
                match specialty {
                    Some(specialty) => stats.record_deliberation(&specialty, duration),
                    None => stats.record_orchestration(duration),
                }
                tx.insert(
                    Table::CouncilStatistics,
                    Key::Str("totals"),
                    &encode(&stats, "record statistics")?,
                )?;
                append(
                    tx.as_mut(),
                    CouncilJournalEvent::StatisticsRecorded(stats),
                    authorization,
                )?;
                tx.commit()
            })
            .await
    }
}
#[async_trait]
impl StatisticsPort for SqliteCouncilStatistics {
    async fn record_deliberation(
        &self,
        specialty: &Specialty,
        duration: DurationMs,
    ) -> Result<(), DomainError> {
        self.record(Some(specialty.clone()), duration).await
    }
    async fn record_orchestration(&self, duration: DurationMs) -> Result<(), DomainError> {
        self.record(None, duration).await
    }
    async fn snapshot(&self) -> Result<Statistics, DomainError> {
        self.store
            .blocking(|engine| {
                engine
                    .begin_read()?
                    .get(Table::CouncilStatistics, Key::Str("totals"))?
                    .map(|bytes| decode(&bytes, "read statistics"))
                    .transpose()
                    .map(Option::unwrap_or_default)
            })
            .await
    }
}
