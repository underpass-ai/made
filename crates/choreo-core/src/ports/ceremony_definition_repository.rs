//! [`CeremonyDefinitionRepositoryPort`] — persistence for ceremony definitions.

use async_trait::async_trait;

use crate::entities::CeremonyDefinition;
use crate::error::DomainError;
use crate::value_objects::{CeremonyName, CeremonyVersion};

#[async_trait]
pub trait CeremonyDefinitionRepositoryPort: Send + Sync {
    async fn save(&self, definition: &CeremonyDefinition) -> Result<(), DomainError>;

    async fn get(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError>;

    async fn list(&self) -> Result<Vec<CeremonyDefinition>, DomainError>;
}
