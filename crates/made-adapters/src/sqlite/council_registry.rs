use super::ceremony_store::encode;
use super::council_store::append;
use super::SqliteCouncilStore;
use crate::engine::{Key, Table};
use async_trait::async_trait;
use made_core::entities::{Council, CouncilJournalEvent};
use made_core::error::DomainError;
use made_core::ports::CouncilRegistryPort;
use made_core::value_objects::{AuthorizationEvidence, Specialty};

#[derive(Debug, Clone)]
pub struct SqliteCouncilRegistry {
    store: SqliteCouncilStore,
}
impl SqliteCouncilRegistry {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
}
#[async_trait]
impl CouncilRegistryPort for SqliteCouncilRegistry {
    async fn register(&self, council: Council) -> Result<(), DomainError> {
        self.register_authorized(council, None).await
    }
    async fn register_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        self.store
            .insert(
                Table::Councils,
                council.specialty().to_string(),
                council.clone(),
                "council",
                CouncilJournalEvent::CouncilRegistered(council),
                authorization,
            )
            .await
    }
    async fn replace(&self, council: Council) -> Result<(), DomainError> {
        self.replace_authorized(council, None).await
    }
    async fn replace_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let key = council.specialty().as_str();
                if tx.get(Table::Councils, Key::Str(key))?.is_none() {
                    return Err(DomainError::NotFound { what: "council" });
                }
                tx.insert(
                    Table::Councils,
                    Key::Str(key),
                    &encode(&council, "replace council")?,
                )?;
                append(
                    tx.as_mut(),
                    CouncilJournalEvent::CouncilReplaced(council),
                    authorization,
                )?;
                tx.commit()
            })
            .await
    }
    async fn get(&self, specialty: &Specialty) -> Result<Council, DomainError> {
        self.store
            .get(Table::Councils, specialty.to_string(), "council")
            .await
    }
    async fn list(&self) -> Result<Vec<Council>, DomainError> {
        self.store.list(Table::Councils).await
    }
    async fn delete(&self, specialty: &Specialty) -> Result<(), DomainError> {
        self.delete_authorized(specialty, None).await
    }
    async fn delete_authorized(
        &self,
        specialty: &Specialty,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        self.store
            .delete(
                Table::Councils,
                specialty.to_string(),
                "council",
                CouncilJournalEvent::CouncilDeleted(specialty.clone()),
                authorization,
            )
            .await
    }
    async fn contains(&self, specialty: &Specialty) -> Result<bool, DomainError> {
        self.store
            .contains(Table::Councils, specialty.to_string())
            .await
    }
}
